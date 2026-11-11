// @Time    : 2026/11/11
// @Author  : fzf
// @FileName: pty_read.rs
// @Software: Siskin

//! 自有 PTY 读线程，替代 `alacritty_terminal` 的 `EventLoop`。
//!
//! 替换的原因只有一个：上游解析层把 OSC 133 当未知序列丢弃，不产生
//! 任何事件；要拿到带行号的命令标记，必须自己读 PTY、自己调解析器。
//! 除此之外的行为都与上游 `EventLoop::spawn`（alacritty_terminal 0.26.0
//! `src/event_loop.rs`）逐项对齐：
//!
//! - PTY 主端非阻塞读，读满 64KB 让出一次终端锁（上游 `MAX_LOCKED_READ`）；
//! - 读到的字节全部原样喂给 `Processor::advance`，一字不改；
//! - 非同步字节才发 `Wakeup`（`sync_bytes_count() < processed`）；
//! - `Msg::Input` 分段写回，`WouldBlock` 留待下次 `POLLOUT`；
//! - 子进程退出先 drain 残留输出、再发 `ChildExit`、`Term::exit()`、发
//!   `Wakeup`。与上游有一处不同（上游先发退出再 drain）：消费端可能
//!   在 drain 完成前看到退出 + 空队列就去读帧，尾部还没解析，
//!   `seq 1M` 的 2ms 测试在并行负载下因此抖动失败过；
//!   先 drain 再发退出，看到退出时输出已全部落进网格；
//! - 同步更新超时（`sync_timeout`）到达时 `stop_sync` 并发 `Wakeup`。
//!
//! 多出来的只有 OSC 133 扫描：先扫整段 buf 找标记偏移，再按段推进——
//! 每到一个标记偏移处停下，读锁内光标行发出带行号的标记，再继续推进。

use std::borrow::Cow;
use std::collections::VecDeque;
use std::io::{ErrorKind, Read, Write};
use std::os::unix::io::{AsRawFd, RawFd};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::thread::JoinHandle;
use std::time::Instant;

use alacritty_terminal::Term;
use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::tty::{self, ChildEvent, EventedPty, EventedReadWrite};
use alacritty_terminal::vte::ansi::Processor;

use super::osc133::Osc133Scanner;
use super::pty_channel::{PtyChannel, PtyMsg, new_channel};
use super::vt::VtListener;

/// 单次 `read` 最多读的字节数，也是单次持锁解析的上限，
/// 与上游 `MAX_LOCKED_READ`（`u16::MAX`）一致。
const READ_CHUNK: usize = u16::MAX as usize;

/// 无同步超时时 `poll` 的等待上限（毫秒）：只为及时发现子进程退出，
/// `poll` 有数据或消息时会立即返回，不影响输入延迟。
const CHILD_POLL_MS: i32 = 200;

/// 启动读线程。`tty` 归线程所有，退出时 drop 会给子进程发 SIGHUP。
/// 管道或线程创建失败时返回 IO 错误，由 `Pty::spawn` 转成 `PtyError`。
pub(crate) fn spawn_reader(
    terminal: Arc<FairMutex<Term<VtListener>>>,
    listener: VtListener,
    tty: tty::Pty,
) -> std::io::Result<(PtyChannel, JoinHandle<()>)> {
    let (channel, receiver, wake_read) = new_channel()?;
    let master_fd = tty.file().as_raw_fd();
    let handle = std::thread::Builder::new()
        .name("PTY reader".to_string())
        .spawn(move || {
            reader_loop(terminal, listener, tty, master_fd, wake_read, receiver);
        })?;
    Ok((channel, handle))
}

/// 未写完的输入。`Writing` 与分段写逻辑照搬上游 `State`，
/// 避免大数据粘贴时半截丢失或错序。
struct Writing {
    source: Cow<'static, [u8]>,
    written: usize,
}

impl Writing {
    fn new(source: Cow<'static, [u8]>) -> Self {
        Self { source, written: 0 }
    }

    fn remaining_bytes(&self) -> &[u8] {
        &self.source[self.written..]
    }

    fn advance(&mut self, count: usize) {
        self.written += count;
    }

    fn finished(&self) -> bool {
        self.written >= self.source.len()
    }
}

struct ReaderState {
    parser: Processor,
    scanner: Osc133Scanner,
    write_list: VecDeque<Cow<'static, [u8]>>,
    writing: Option<Writing>,
    buf: Vec<u8>,
}

impl ReaderState {
    fn new() -> Self {
        Self {
            parser: Processor::new(),
            scanner: Osc133Scanner::new(),
            write_list: VecDeque::new(),
            writing: None,
            buf: vec![0; READ_CHUNK],
        }
    }

    fn needs_write(&self) -> bool {
        self.writing.is_some() || !self.write_list.is_empty()
    }
}

fn reader_loop(
    terminal: Arc<FairMutex<Term<VtListener>>>,
    listener: VtListener,
    mut tty: tty::Pty,
    master_fd: RawFd,
    wake_read: RawFd,
    receiver: Receiver<PtyMsg>,
) {
    let mut state = ReaderState::new();
    loop {
        let sync_timeout = state.parser.sync_timeout().sync_timeout();
        let timeout = sync_timeout.map_or(CHILD_POLL_MS, |deadline| {
            deadline
                .saturating_duration_since(Instant::now())
                .as_millis()
                .min(i32::MAX as u128) as i32
        });
        let mut fds = poll_fds(master_fd, wake_read, state.needs_write());
        // SAFETY: `fds` 是栈上合法数组，`timeout` 是毫秒数。
        let ready = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, timeout) };
        if ready < 0 {
            if std::io::Error::last_os_error().kind() == ErrorKind::Interrupted {
                continue;
            }
            break;
        }
        if !poll_once(
            &terminal,
            &listener,
            &mut tty,
            wake_read,
            &receiver,
            &mut state,
            &fds,
            ready,
            sync_timeout,
        ) {
            break;
        }
    }
    // SAFETY: 管道读端只在这里关闭一次。
    unsafe { libc::close(wake_read) };
}

/// 一轮 poll 之后的处理：消息、读、写、同步超时、子进程退出。
/// 返回 `false` 时线程退出。
#[allow(clippy::too_many_arguments)]
fn poll_once(
    terminal: &Arc<FairMutex<Term<VtListener>>>,
    listener: &VtListener,
    tty: &mut tty::Pty,
    wake_read: RawFd,
    receiver: &Receiver<PtyMsg>,
    state: &mut ReaderState,
    fds: &[libc::pollfd; 2],
    ready: i32,
    sync_timeout: Option<Instant>,
) -> bool {
    if drain_messages(receiver, state) {
        return false;
    }
    if ready > 0 && fds[1].revents & libc::POLLIN != 0 {
        drain_wake_pipe(wake_read);
    }
    let mut eio = false;
    if ready > 0 && fds[0].revents & libc::POLLIN != 0 {
        match read_and_parse(terminal, listener, state, tty) {
            Ok(()) => {}
            Err(error) if error.raw_os_error() == Some(libc::EIO) => eio = true,
            Err(_) => return false,
        }
    }
    if ready > 0
        && (fds[0].revents & libc::POLLOUT != 0 || state.needs_write())
        && write_pending(tty, state).is_err()
    {
        return false;
    }
    if ready == 0
        && sync_timeout.is_some()
        && let Some(mut term) = terminal.try_lock_unfair()
    {
        state.parser.stop_sync(&mut *term);
        listener.send_event(Event::Wakeup);
    }
    if eio {
        // EIO 只说明主端挂起，不等于子进程已死：探一次，没死
        // 就等下一次 poll，避免对着一个已挂的 fd 空转。
        if let Some(exit) = tty.next_child_event() {
            finish_exit(terminal, listener, state, tty, exit);
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    } else if let Some(exit) = tty.next_child_event() {
        finish_exit(terminal, listener, state, tty, exit);
        return false;
    }
    true
}

fn poll_fds(master_fd: RawFd, wake_read: RawFd, want_write: bool) -> [libc::pollfd; 2] {
    [
        libc::pollfd {
            fd: master_fd,
            events: if want_write {
                libc::POLLIN | libc::POLLOUT
            } else {
                libc::POLLIN
            },
            revents: 0,
        },
        libc::pollfd {
            fd: wake_read,
            events: libc::POLLIN,
            revents: 0,
        },
    ]
}

/// 排空消息通道。收到 `Shutdown` 返回 `true`。
fn drain_messages(receiver: &Receiver<PtyMsg>, state: &mut ReaderState) -> bool {
    loop {
        match receiver.try_recv() {
            Ok(PtyMsg::Input(input)) => state.write_list.push_back(input),
            Ok(PtyMsg::Shutdown) => return true,
            Err(TryRecvError::Empty) => return false,
            Err(TryRecvError::Disconnected) => return true,
        }
    }
}

fn drain_wake_pipe(wake_read: RawFd) {
    let mut byte = [0u8; 64];
    loop {
        // SAFETY: 非阻塞管道，读空返回 -1，不阻塞线程。
        let got = unsafe { libc::read(wake_read, byte.as_mut_ptr() as *mut libc::c_void, 64) };
        if got <= 0 {
            break;
        }
    }
}

/// 分段写回排队的输入：`WouldBlock` 时留待下次 `POLLOUT`，
/// 其它 IO 错误交给调用方结束线程。
fn write_pending(tty: &mut tty::Pty, state: &mut ReaderState) -> std::io::Result<()> {
    if state.writing.is_none() {
        state.writing = state.write_list.pop_front().map(Writing::new);
    }
    while let Some(mut current) = state.writing.take() {
        match tty.writer().write(current.remaining_bytes()) {
            Ok(0) => {
                state.writing = Some(current);
                break;
            }
            Ok(written) => {
                current.advance(written);
                if !current.finished() {
                    state.writing = Some(current);
                    break;
                }
            }
            Err(error) => {
                state.writing = Some(current);
                match error.kind() {
                    ErrorKind::Interrupted | ErrorKind::WouldBlock => break,
                    _ => return Err(error),
                }
            }
        }
    }
    Ok(())
}

/// 读一段 PTY 输出：先扫 OSC 133 标记，再按段推进解析器。
/// 每到一个标记偏移处停下，读锁内光标行发出带行号的标记。
fn read_and_parse(
    terminal: &Arc<FairMutex<Term<VtListener>>>,
    listener: &VtListener,
    state: &mut ReaderState,
    tty: &mut tty::Pty,
) -> std::io::Result<()> {
    let mut processed = 0usize;
    loop {
        match tty.reader().read(&mut state.buf) {
            Ok(0) => break,
            Ok(got) => {
                let chunk = &state.buf[..got];
                let hits = state.scanner.scan(chunk);
                let mut term = terminal.lock();
                let mut pos = 0usize;
                for hit in &hits {
                    if hit.anchor > pos {
                        state.parser.advance(&mut *term, &chunk[pos..hit.anchor]);
                    }
                    // 行号必须是单调稳定的：`HistorySpan` 式行号会随新
                    // 输出到来整体下移（旧内容滚进历史），存下来就漂移。
                    // 稳定行 = history_size + 绝对行，绝对行 = 视口行
                    // - display_offset；跳转时再用当前 history_size 换回。
                    // 列同样在解析标记字节之前读：`B` 到达时光标正在
                    // 提示符末尾，这个列就是 ghost 的输入起点。
                    let line = term.history_size() as i32 + term.grid().cursor.point.line.0
                        - term.grid().display_offset() as i32;
                    let column = term.grid().cursor.point.column.0 as i32;
                    listener.shell_mark(line, column, hit.mark);
                    state
                        .parser
                        .advance(&mut *term, &chunk[hit.anchor..hit.end]);
                    pos = hit.end;
                }
                state.parser.advance(&mut *term, &chunk[pos..]);
                drop(term);
                processed += got;
                if processed >= READ_CHUNK {
                    break;
                }
            }
            Err(error) => match error.kind() {
                ErrorKind::Interrupted | ErrorKind::WouldBlock => break,
                _ => return Err(error),
            },
        }
    }
    // 与上游一致：全是同步字节时不发 Wakeup，避免无谓构帧。
    if state.parser.sync_bytes_count() < processed && processed > 0 {
        listener.send_event(Event::Wakeup);
    }
    Ok(())
}

/// 子进程退出：先 drain 残留输出，再发 `ChildExit`，
/// 然后 `Term::exit()`、发 `Wakeup`。
///
/// 顺序与上游有一处不同：上游先发 `ChildExit` 再 drain，消费端可能
/// 在 drain 完成前看到退出 + 空队列就去读帧，尾部字节还没解析
/// （`seq 1M` 的 2ms 测试在并行负载下因此抖动失败过）。
/// 先 drain 再发退出，看到退出时输出已全部落进网格。
fn finish_exit(
    terminal: &Arc<FairMutex<Term<VtListener>>>,
    listener: &VtListener,
    state: &mut ReaderState,
    tty: &mut tty::Pty,
    exit: ChildEvent,
) {
    let _ = read_and_parse(terminal, listener, state, tty);
    if let ChildEvent::Exited(Some(status)) = exit {
        listener.send_event(Event::ChildExit(status));
    }
    terminal.lock().exit();
    listener.send_event(Event::Wakeup);
}
