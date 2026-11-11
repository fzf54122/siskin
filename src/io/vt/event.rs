// @Time    : 2026/11/5 19:05
// @Author  : fzf
// @FileName: event.rs
// @Software: Siskin

//! 后端事件到应用事件的翻译。PTY 读线程与主线程共用一条带会话 id 的
//! 通道，这样关闭会话时不需要逐个注销。

use alacritty_terminal::event::{Event, EventListener, WindowSize};
use alacritty_terminal::grid::Dimensions;

use crate::domain::shell_marks::{Osc133Mark, ShellMark};

/// 终端上报给应用的事件。
#[derive(Clone)]
pub(crate) enum VtEvent {
    Wakeup,
    Title(String),
    ResetTitle,
    Bell,
    ClipboardStore(String),
    /// 程序要求终端回应一段序列（光标位置报告、设备属性等）。
    PtyWrite(String),
    ChildExit(Option<i32>),
    Exit,
    /// OSC 133 标记，带标记到达时刻的稳定行号（从会话第一行起算的
    /// 单调行计数，跳转时用当前 `history_size` 换回绝对行）。
    ShellMark(ShellMark),
}

impl std::fmt::Debug for VtEvent {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VtEvent::Wakeup => write!(formatter, "Wakeup"),
            VtEvent::Title(title) => write!(formatter, "Title({title})"),
            VtEvent::ResetTitle => write!(formatter, "ResetTitle"),
            VtEvent::Bell => write!(formatter, "Bell"),
            VtEvent::ClipboardStore(text) => {
                write!(formatter, "ClipboardStore({} bytes)", text.len())
            }
            VtEvent::PtyWrite(_) => write!(formatter, "PtyWrite"),
            VtEvent::ChildExit(code) => write!(formatter, "ChildExit({code:?})"),
            VtEvent::Exit => write!(formatter, "Exit"),
            VtEvent::ShellMark(mark) => {
                write!(formatter, "ShellMark(line {}, {:?})", mark.line, mark.mark)
            }
        }
    }
}

#[derive(Clone)]
pub(crate) struct VtListener {
    sender: async_channel::Sender<(u64, VtEvent)>,
    session: u64,
}

impl VtListener {
    pub(crate) fn new(sender: async_channel::Sender<(u64, VtEvent)>, session: u64) -> Self {
        Self { sender, session }
    }

    fn emit(&self, event: VtEvent) {
        // 接收端消失意味着应用正在退出，此时丢弃事件是正确行为。
        let _ = self.sender.try_send((self.session, event));
    }

    /// 读线程专用：OSC 133 标记不是上游 `Event`，不走 `send_event`，
    /// 直接按同一个通道发回应用层。
    pub(crate) fn shell_mark(&self, line: i32, mark: Osc133Mark) {
        self.emit(VtEvent::ShellMark(ShellMark::new(line, mark)));
    }
}

impl EventListener for VtListener {
    fn send_event(&self, event: Event) {
        let mapped = match event {
            Event::Wakeup => VtEvent::Wakeup,
            Event::Title(title) => VtEvent::Title(title),
            Event::ResetTitle => VtEvent::ResetTitle,
            Event::Bell => VtEvent::Bell,
            Event::ClipboardStore(_, text) => VtEvent::ClipboardStore(text),
            Event::PtyWrite(text) => VtEvent::PtyWrite(text),
            Event::ChildExit(status) => VtEvent::ChildExit(status.code()),
            Event::Exit => VtEvent::Exit,
            // 光标闪烁、鼠标形状、尺寸查询与颜色查询都不需要主线程处理。
            _ => return,
        };
        self.emit(mapped);
    }
}

/// 网格尺寸。`Dimensions` 要求总行数包含滚动区。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ScreenSize {
    pub columns: usize,
    pub rows: usize,
    pub history: usize,
}

impl ScreenSize {
    pub(crate) fn new(columns: usize, rows: usize, history: usize) -> Self {
        Self {
            columns: columns.max(2),
            rows: rows.max(1),
            history,
        }
    }

    pub(crate) fn window_size(self, cell_width: u16, cell_height: u16) -> WindowSize {
        WindowSize {
            num_lines: self.rows.min(u16::MAX as usize) as u16,
            num_cols: self.columns.min(u16::MAX as usize) as u16,
            cell_width,
            cell_height,
        }
    }
}

impl Dimensions for ScreenSize {
    fn total_lines(&self) -> usize {
        self.rows + self.history
    }

    fn screen_lines(&self) -> usize {
        self.rows
    }

    fn columns(&self) -> usize {
        self.columns
    }
}
