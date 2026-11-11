// @Time    : 2026/11/5 16:20
// @Author  : fzf
// @FileName: pty.rs
// @Software: Siskin

//! PTY 会话。派生进程、写端、读线程与 resize 都在这里，主线程只碰
//! `write` / `resize` 和事件通道。
//!
//! 线程模型：自有读线程（`super::pty_read`）阻塞读，解析结果写进
//! `FairMutex<Term>`；主线程只在渲染前短暂加锁，不跨 await。
//! 不用上游 `EventLoop` 只有一个原因：OSC 133 需要在解析前扫字节流
//! 并按标记偏移锚定光标行，上游循环不给这个钩子。

use std::borrow::Cow;
use std::sync::Arc;

use alacritty_terminal::Term;
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::TermMode;
use alacritty_terminal::tty::{self, Options, Shell};

use super::history_search::HistorySpan;
use super::pty_channel::{PtyChannel, PtyMsg};
use super::pty_read::spawn_reader;
use super::vt::{ScreenSize, VtEvent, VtListener};
use crate::domain::shell_spec::ShellSpec;

/// 派生失败的原因。`Spawn` 保留原始 IO 错误，`Shell` 是启动前的校验失败。
#[derive(Debug)]
pub(crate) enum PtyError {
    Shell(String),
    Spawn(std::io::Error),
}

impl std::fmt::Display for PtyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PtyError::Shell(message) => write!(formatter, "{message}"),
            PtyError::Spawn(error) => write!(formatter, "无法启动终端进程：{error}"),
        }
    }
}

impl std::error::Error for PtyError {}

/// 一个已经跑起来的 PTY。面板退出时 drop，由 Drop 关闭事件循环。
pub(crate) struct Pty {
    terminal: Arc<FairMutex<Term<VtListener>>>,
    sender: PtyChannel,
    size: ScreenSize,
    /// 子进程退出后不再接受输入，但网格仍可读，供用户回看最后输出。
    exited: bool,
    /// 子进程 pid，用于读 cwd 与排查启动问题。
    child_pid: Option<i32>,
    /// PTY 主端 fd。`TIOCSWINSZ` 直接作用在这里，不经过事件循环队列。
    master_fd: std::os::unix::io::RawFd,
    /// 读线程。Drop 时先发 `Shutdown` 再 join，保证句柄与 PTY 都被释放。
    reader: Option<std::thread::JoinHandle<()>>,
}

impl Pty {
    /// 派生进程并启动读线程。`events` 用来把 VT 事件送回 UI 线程。
    pub(crate) fn spawn(
        spec: &ShellSpec,
        working_directory: Option<std::path::PathBuf>,
        size: ScreenSize,
        events: async_channel::Sender<(u64, VtEvent)>,
        session: u64,
        window_id: u64,
        env: &[(String, String)],
    ) -> Result<Self, PtyError> {
        let resolved = spec
            .clone()
            .resolve()
            .map_err(|error| PtyError::Shell(error.to_string()))?;
        let options = Options {
            shell: Some(Shell::new(resolved.program, resolved.args)),
            working_directory,
            drain_on_exit: true,
            env: env.iter().cloned().collect(),
        };
        let window_size = size.window_size(8, 16);
        let tty = tty::new(&options, window_size, window_id)
            .map_err(|error| PtyError::Spawn(std::io::Error::other(error.to_string())))?;
        let child_pid = Some(tty.child().id() as i32);

        // master 端 fd 用于同步设置窗口尺寸，见 `resize`。必须在把 `tty`
        // 交给事件循环之前取，之后它归读线程所有。
        let master_fd = tty_fd(&tty);

        let listener = VtListener::new(events, session);
        let config = alacritty_terminal::term::Config {
            scrolling_history: size.history,
            osc52: alacritty_terminal::term::Osc52::OnlyCopy,
            ..Default::default()
        };
        let terminal = Arc::new(FairMutex::new(Term::new(config, &size, listener.clone())));
        // 自有读线程：`EventLoop` 的行为在 `pty_read` 里逐项对齐，
        // 多出来的只有 OSC 133 扫描与行号锚定。
        let (sender, reader) =
            spawn_reader(terminal.clone(), listener, tty).map_err(PtyError::Spawn)?;

        Ok(Self {
            terminal,
            sender,
            size,
            exited: false,
            child_pid,
            master_fd,
            reader: Some(reader),
        })
    }

    #[cfg(test)]
    pub(crate) fn size(&self) -> ScreenSize {
        self.size
    }

    pub(crate) fn write(&mut self, bytes: &[u8]) {
        if self.exited || bytes.is_empty() {
            return;
        }
        if !self.sender.send(PtyMsg::Input(Cow::Owned(bytes.to_vec()))) {
            eprintln!("siskin: 无法写入终端：读线程已退出");
        }
    }

    pub(crate) fn paste(&mut self, text: &str) {
        let bracketed = self.mode().contains(TermMode::BRACKETED_PASTE);
        let bytes = super::keys::wrap_paste(text, bracketed);
        self.write(&bytes);
    }

    /// 调整尺寸。`TIOCSWINSZ` 直接对主端 fd 调用，而不是投递 `Msg::Resize`：
    /// 后者依赖 poller 唤醒，唤醒若发生在事件循环进入 `poll` 之前就会丢失，
    /// 表现为「偶尔 resize 不生效」。ioctl 是同步的，没有这个窗口。
    ///
    /// 顺序上先让内核知道新尺寸，再更新本地网格，避免程序按旧尺寸先画一屏。
    pub(crate) fn resize(&mut self, size: ScreenSize) {
        if size == self.size {
            return;
        }
        if let Err(error) = set_window_size(self.master_fd, size) {
            eprintln!("siskin: 无法调整终端尺寸：{error}");
        }
        self.terminal.lock().resize(size);
        self.size = size;
    }

    pub(crate) fn mark_exited(&mut self) {
        self.exited = true;
    }

    pub(crate) fn mode(&self) -> TermMode {
        *self.terminal.lock().mode()
    }

    /// 在锁内构造一帧，返回后立即释放锁，渲染阶段不再加锁。
    pub(crate) fn frame(&self, palette: &super::ansi::AnsiColors) -> super::vt::Frame {
        super::vt::frame(&self.terminal.lock(), palette)
    }

    pub(crate) fn scroll(&mut self, delta: i32) {
        self.terminal
            .lock()
            .scroll_display(alacritty_terminal::grid::Scroll::Delta(delta));
    }

    /// 当前滚动偏移（帧顶相对可视区首行）。命令跳转用它算目标行。
    pub(crate) fn display_offset(&self) -> usize {
        self.terminal.lock().grid().display_offset()
    }

    /// 滚动区里的行数。命令跳转用它把稳定行号换回绝对行。
    pub(crate) fn history_size(&self) -> usize {
        use alacritty_terminal::grid::Dimensions;
        self.terminal.lock().history_size()
    }

    /// 锁内扫完整 scrollback（含历史行），一次返回全部匹配。
    /// 无效正则返回构建错误文案，不抛错、不清空调用方状态。
    pub(crate) fn search_history(
        &self,
        query: &str,
        case_sensitive: bool,
        regex: bool,
    ) -> Result<Vec<HistorySpan>, String> {
        super::history_search::search_history(&self.terminal.lock(), query, case_sensitive, regex)
    }

    /// 子进程工作目录。Linux 下读 `/proc/<pid>/cwd`，失败返回 `None`
    /// 而不是报错：面板信息缺一行不影响终端使用。
    pub(crate) fn working_directory(&self) -> Option<std::path::PathBuf> {
        let pid = self.child_pid?;
        std::fs::read_link(format!("/proc/{pid}/cwd")).ok()
    }

    pub(crate) fn has_running_job(&self) -> bool {
        if self.exited {
            return false;
        }
        let Some(pid) = self.child_pid else {
            return true;
        };
        // 只认前台进程组。子进程列表不能用作判据：Powerlevel10k 之类的
        // 提示符会在后台常驻 gitstatus 守护进程，那不代表有任务在跑。
        // SAFETY: 只查询仍由此 PTY 持有的主端，不修改终端或进程组。
        let foreground = unsafe { libc::tcgetpgrp(self.master_fd) };
        if foreground <= 0 {
            // 主端已失效时保守处理，宁可按「有任务」提示一次。
            return true;
        }
        let foreground = foreground as i32;
        if foreground == pid {
            return false;
        }
        // shell 自己 fork 出前台子进程时，前台进程组是子进程组；
        // 只要不是 shell 本人的进程组就算有任务。
        let shell_group = unsafe { libc::getpgid(pid) };
        shell_group <= 0 || foreground != shell_group
    }

    pub(crate) fn selection_text(&self) -> Option<String> {
        self.terminal.lock().selection_to_string()
    }

    /// `row` 是可视区行号（0 是最上面一行），内部换算成网格绝对行号。
    pub(crate) fn select(&mut self, row: isize, column: usize) {
        use alacritty_terminal::index::{Column, Line, Point, Side};
        use alacritty_terminal::selection::{Selection, SelectionType};
        let mut term = self.terminal.lock();
        let line = Line(row as i32 - term.grid().display_offset() as i32);
        term.selection = Some(Selection::new(
            SelectionType::Simple,
            Point::new(line, Column(column)),
            Side::Left,
        ));
    }

    pub(crate) fn update_selection(&mut self, row: isize, column: usize) {
        use alacritty_terminal::index::{Column, Line, Point, Side};
        let mut term = self.terminal.lock();
        let line = Line(row as i32 - term.grid().display_offset() as i32);
        if let Some(selection) = term.selection.as_mut() {
            selection.update(Point::new(line, Column(column)), Side::Right);
        }
    }

    pub(crate) fn clear_selection(&mut self) {
        self.terminal.lock().selection = None;
    }
}

/// PTY 主端 fd。`tty::Pty` 在 spawn 之后由事件循环线程持有，
/// 这里在创建时先取一份裸 fd 供 resize 使用。
fn tty_fd(tty: &alacritty_terminal::tty::Pty) -> std::os::unix::io::RawFd {
    use std::os::unix::io::AsRawFd;
    tty.file().as_raw_fd()
}

fn set_window_size(fd: std::os::unix::io::RawFd, size: ScreenSize) -> std::io::Result<()> {
    let window = size.window_size(8, 16);
    let winsize = libc::winsize {
        ws_row: window.num_lines,
        ws_col: window.num_cols,
        ws_xpixel: window.cell_width.saturating_mul(window.num_cols),
        ws_ypixel: window.cell_height.saturating_mul(window.num_lines),
    };
    // SAFETY: `fd` 是 PTY 主端，`winsize` 在调用期间有效；ioctl 不持有引用。
    let result = unsafe { libc::ioctl(fd, libc::TIOCSWINSZ, &winsize) };
    if result == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

impl Drop for Pty {
    fn drop(&mut self) {
        // 读线程收到 Shutdown 后退出并释放 PTY，子进程随之收到 SIGHUP。
        // 先发消息再 join：顺序反了会让 join 永久阻塞在 poll 上。
        let _ = self.sender.send(PtyMsg::Shutdown);
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}
