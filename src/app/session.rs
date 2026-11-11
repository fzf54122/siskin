// @Time    : 2026/11/5 16:40
// @Author  : fzf
// @FileName: session.rs
// @Software: Siskin

//! 会话仓储。每个会话持有一个真实 PTY，`SiskinApp` 只调这里的方法，
//! 不直接索引 `Vec`，窗口关闭由应用层处理。

use std::sync::Arc;

use gpui::SharedString;

use crate::domain::SessionSpec;
use crate::domain::session::SessionState;
use crate::domain::shell_marks::CommandLog;
use crate::domain::shell_spec::ShellKind;
use crate::io::ansi::AnsiColors;
use crate::io::pty::Pty;
use crate::io::vt::{Frame, ScreenSize, VtEvent};

/// 新会话先按 80×24 派生，首帧拿到真实尺寸后再 resize 一次。
pub(crate) const INITIAL_COLUMNS: usize = 80;
pub(crate) const INITIAL_ROWS: usize = 24;
pub(crate) const SCROLLBACK_LINES: usize = 10_000;

pub(crate) struct Session {
    pub id: u64,
    pub title: SharedString,
    /// 用户显式改的名。`None` 表示没改过，`display_title` 才回退到目录名；
    /// 有值时哪怕是「终端」也照显，不再被目录名覆盖。
    pub custom_title: Option<String>,
    pub kind: ShellKind,
    pub state: SessionState,
    pub directory: Option<std::path::PathBuf>,
    pub window_title: Option<String>,
    pty: Option<Pty>,
    /// IME 预编辑串。只有提交时才写入 PTY。
    pub marked: Option<String>,
    /// 共享快照：`Frame` 不可变，渲染与搜索各持一份 `Arc` 指向同一帧，
    /// 不再每次深拷贝行与 cell；替换只是换指针，旧快照仍完整可读。
    pub frame: Arc<Frame>,
    pub columns: usize,
    pub rows: usize,
    pub panes: crate::app::pane::PaneTree, // 分屏树：叶指向会话 id，初始单叶是自身。
    /// 平滑滚动的亚行余量。整行由 PTY 持有，余量只活在渲染层。
    pub scroll_offset: crate::domain::ScrollOffset,
    /// 最近一轮事件是否有输出。只在有 PTY 时置位，见 `mark_output`。
    frame_dirty: bool,
    /// OSC 133 命令记录：每会话独立，上限 100 条。无标记时全空是合法态。
    pub shell_marks: CommandLog,
}

impl Session {
    pub(crate) fn starting(id: u64, title: impl Into<SharedString>, kind: ShellKind) -> Self {
        Self {
            id,
            title: title.into(),
            custom_title: None,
            kind,
            state: SessionState::Starting,
            directory: std::env::current_dir().ok(),
            window_title: None,
            pty: None,
            marked: None,
            frame: Arc::new(Frame::empty(INITIAL_ROWS)),
            columns: INITIAL_COLUMNS,
            rows: INITIAL_ROWS,
            panes: crate::app::pane::PaneTree::leaf(id),
            scroll_offset: crate::domain::ScrollOffset::default(),
            frame_dirty: false,
            shell_marks: CommandLog::new(),
        }
    }

    pub(crate) fn spawn_spec(
        &mut self,
        spec: &SessionSpec,
        events: async_channel::Sender<(u64, VtEvent)>,
        window_id: u64,
    ) -> Result<(), String> {
        let size = ScreenSize::new(self.columns, self.rows, SCROLLBACK_LINES);
        let id = self.id;
        // 显式给终端环境，避免「从桌面启动」和「从终端里启动」拿到不同的
        // `TERM`，导致提示符与颜色表现不一致。
        let env = crate::domain::shell_spec::terminal_env();
        match Pty::spawn(
            &spec.shell,
            spec.working_directory.clone(),
            size,
            events,
            id,
            window_id,
            &env,
        ) {
            Ok(pty) => {
                self.pty = Some(pty);
                self.state = SessionState::Running;
                Ok(())
            }
            Err(error) => {
                self.pty = None;
                let message = error.to_string();
                self.state = SessionState::Failed(message.clone());
                Err(message)
            }
        }
    }

    pub(crate) fn is_running(&self) -> bool {
        self.state.is_running() && self.pty.is_some()
    }

    pub(crate) fn pty(&self) -> Option<&Pty> {
        self.pty.as_ref()
    }

    pub(crate) fn write(&mut self, bytes: &[u8]) {
        if let Some(pty) = self.pty.as_mut() {
            pty.write(bytes);
        }
    }

    pub(crate) fn paste(&mut self, text: &str) {
        if let Some(pty) = self.pty.as_mut() {
            pty.paste(text);
        }
    }

    /// 构帧耗时由调用方记进 P0 采样。没有 PTY 时返回零时长，
    /// 不把「没会话」算成一次昂贵的空帧。
    /// 只在脏时替换 `Arc`：干净会话跳过构帧，旧快照仍可读；
    /// 替换只是换指针，渲染与搜索手里的旧 `Arc` 不受影响，
    /// 因此永远读不到半更新帧。
    pub(crate) fn refresh(&mut self, colors: &AnsiColors) -> std::time::Duration {
        let started = std::time::Instant::now();
        if self.frame_dirty
            && let Some(pty) = self.pty.as_ref()
        {
            self.frame = Arc::new(pty.frame(colors));
            self.frame_dirty = false;
        }
        started.elapsed()
    }

    /// 共享快照：渲染与搜索拿同一份 `Arc`，不重复深拷贝整帧。
    pub(crate) fn frame_snapshot(&self) -> Arc<Frame> {
        Arc::clone(&self.frame)
    }

    /// 从 PTY 收到输出后调用。构帧由事件轮次的 `refresh` 统一做，
    /// 这样一轮里多个事件只构一次帧，空闲会话完全不碰锁。
    pub(crate) fn mark_output(&mut self) {
        self.mark_frame_dirty();
    }

    /// 标记当前 Frame 需要重建，覆盖滚动、选区和 resize 这类不经 PTY
    /// Wakeup 的界面操作。
    pub(crate) fn mark_frame_dirty(&mut self) {
        if self.pty.is_some() {
            self.frame_dirty = true;
        }
    }

    /// Frame 是否是最近一轮事件或界面操作后的旧值。
    pub(crate) fn frame_dirty(&self) -> bool {
        self.frame_dirty
    }

    pub(crate) fn resize(&mut self, columns: usize, rows: usize) {
        if columns == self.columns && rows == self.rows {
            return;
        }
        self.columns = columns.max(2);
        self.rows = rows.max(1);
        if let Some(pty) = self.pty.as_mut() {
            pty.resize(ScreenSize::new(self.columns, self.rows, SCROLLBACK_LINES));
            self.frame_dirty = true;
        }
    }

    pub(crate) fn mark_exited(&mut self, code: Option<i32>) {
        self.state = SessionState::Exited(crate::domain::ExitInfo::from_status(code));
        if let Some(pty) = self.pty.as_mut() {
            pty.mark_exited();
        }
    }

    pub(crate) fn scroll(&mut self, delta: i32) {
        if let Some(pty) = self.pty.as_mut() {
            pty.scroll(delta);
            self.frame_dirty = true;
        }
    }

    /// 当前滚动偏移（帧顶相对可视区首行）。命令跳转用它算目标行。
    pub(crate) fn display_offset(&self) -> Option<usize> {
        self.pty.as_ref().map(Pty::display_offset)
    }

    /// 滚动区里的行数。命令跳转用它把稳定行号换回绝对行。
    pub(crate) fn history_size(&self) -> Option<usize> {
        self.pty.as_ref().map(Pty::history_size)
    }

    /// 平滑滚动入口。像素位移先攒进亚行余量，凑满一行才提交给 PTY；
    /// 返回值是「是否需要重绘」，余量变化也要重绘才能看出连续性。
    pub(crate) fn scroll_pixels(&mut self, pixels: f32, row_height: f32) -> bool {
        let lines = self.scroll_offset.accumulate(pixels, row_height);
        if lines != 0
            && let Some(pty) = self.pty.as_mut()
        {
            pty.scroll(lines);
            self.frame_dirty = true;
        }
        // 只要有位移（哪怕没凑满一行）就该重绘，否则亚行平移不连续。
        pixels != 0.
    }

    /// 渲染层用的亚行偏移。见 `ScrollOffset::sub_pixel`。
    pub(crate) fn sub_pixel_offset(&self, row_height: f32) -> f32 {
        self.scroll_offset.sub_pixel(row_height)
    }

    pub(crate) fn selection_text(&self) -> Option<String> {
        self.pty.as_ref().and_then(Pty::selection_text)
    }

    pub(crate) fn clear_selection(&mut self) {
        if let Some(pty) = self.pty.as_mut() {
            pty.clear_selection();
            self.frame_dirty = true;
        }
    }

    pub(crate) fn select(&mut self, row: isize, column: usize) {
        if let Some(pty) = self.pty.as_mut() {
            pty.select(row, column);
            self.frame_dirty = true;
        }
    }

    pub(crate) fn update_selection(&mut self, row: isize, column: usize) {
        if let Some(pty) = self.pty.as_mut() {
            pty.update_selection(row, column);
            self.frame_dirty = true;
        }
    }
}

/// 会话集合。保证至少有一个会话，active 始终指向有效下标。
/// `items` / `active` 对 crate 内开放：标签拖拽换位（`tab_ops.rs`）
/// 需要按 id 重算 active，`session.rs` 已到 400 行上限，不再往里加方法。
pub(crate) struct Sessions {
    pub(crate) items: Vec<Session>,
    pub(crate) active: usize,
    next_id: u64,
}

impl Sessions {
    pub(crate) fn new(first: Session) -> Self {
        let next_id = first.id + 1;
        Self {
            items: vec![first],
            active: 0,
            next_id,
        }
    }

    pub(crate) fn next_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub(crate) fn len(&self) -> usize {
        self.items.len()
    }

    pub(crate) fn active_index(&self) -> usize {
        self.active
    }

    pub(crate) fn active(&self) -> &Session {
        &self.items[self.active]
    }

    pub(crate) fn active_mut(&mut self) -> &mut Session {
        &mut self.items[self.active]
    }

    /// 按 id 取可变会话；id 不在仓储里时退回活动标签。
    /// 下标先经 `position` 校验，再直接索引 `items`：两支是同一处
    /// 索引的不同下标，借用检查能证明它们互斥；方法调用版的
    /// `get_mut` / `active_mut` 条件重借用在 NLL 下证不出来。
    pub(crate) fn focused_mut(&mut self, id: u64) -> &mut Session {
        let index = self.position(id).unwrap_or(self.active);
        if index < self.items.len() {
            &mut self.items[index]
        } else {
            &mut self.items[self.active]
        }
    }

    pub(crate) fn get(&self, index: usize) -> Option<&Session> {
        self.items.get(index)
    }

    pub(crate) fn get_mut(&mut self, index: usize) -> Option<&mut Session> {
        self.items.get_mut(index)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (usize, &Session)> {
        self.items.iter().enumerate()
    }

    pub(crate) fn position(&self, id: u64) -> Option<usize> {
        self.items.iter().position(|session| session.id == id)
    }

    pub(crate) fn select(&mut self, index: usize) -> bool {
        if index >= self.items.len() {
            return false;
        }
        self.active = index;
        true
    }

    pub(crate) fn add(&mut self, session: Session) -> usize {
        self.items.push(session);
        self.active = self.items.len() - 1;
        self.active
    }

    /// 最后一个会话由应用关闭窗口处理，不在仓储里重新派生进程。
    pub(crate) fn close(&mut self, index: usize) -> bool {
        if index >= self.items.len() {
            return false;
        }
        if self.items.len() == 1 {
            return false;
        }
        self.items.remove(index);
        if index < self.active {
            self.active -= 1;
        }
        self.active = self.active.min(self.items.len() - 1);
        true
    }

    pub(crate) fn cycle(&mut self, forward: bool) {
        let len = self.items.len();
        if len == 0 {
            return;
        }
        self.active = if forward {
            (self.active + 1) % len
        } else {
            (self.active + len - 1) % len
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sessions(count: usize) -> Sessions {
        let mut set = Sessions::new(Session::starting(0, "第一个", ShellKind::Zsh));
        for index in 1..count {
            let id = set.next_id();
            set.add(Session::starting(
                id,
                format!("会话 {index}"),
                ShellKind::Zsh,
            ));
        }
        set
    }

    #[test]
    fn cycling_wraps_in_both_directions() {
        let mut set = sessions(3);
        set.select(0);
        set.cycle(false);
        assert_eq!(set.active_index(), 2);
        set.cycle(true);
        assert_eq!(set.active_index(), 0);
        assert!(!set.select(9));
    }
}
