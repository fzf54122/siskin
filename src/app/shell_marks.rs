// @Time    : 2026/11/11
// @Author  : fzf
// @FileName: shell_marks.rs
// @Software: Siskin

//! OSC 133 命令跳转：只读每会话的命令记录，按记录滚到目标命令起始行。
//! 与 `Session` 的仓储定义分开，避免 `session.rs` 超 400 行门禁。

use crate::domain::shell_marks::ShellMark;

use super::session::Session;

impl Session {
    /// 消费一个 OSC 133 标记，进命令记录。失败状态只记在记录里，
    /// 不做新面板，下一步的跳转与 UI 再用。
    pub(crate) fn apply_shell_mark(&mut self, mark: ShellMark) {
        self.shell_marks.apply(mark);
    }

    /// 跳到上一个 / 下一个命令的起始行：目标行滚到可见区顶部。
    /// 锚点是视口顶行（稳定行号），不是光标行：从底部往上跳时，
    /// 最新命令的起始行也在锚点之前，必须能跳到。
    /// 没有可跳的命令时返回 `false`，不改滚动位置。
    pub(crate) fn jump_command(&mut self, forward: bool) -> bool {
        let (Some(offset), Some(history)) = (self.display_offset(), self.history_size()) else {
            return false;
        };
        let top = history as i32 - offset as i32;
        let target = if forward {
            self.shell_marks.next_after(top)
        } else {
            self.shell_marks.prev_before(top)
        };
        let Some(span) = target else {
            return false;
        };
        let target_offset = (history as i32 - span.start_line).clamp(0, history as i32);
        self.scroll(target_offset - offset as i32);
        true
    }
}
