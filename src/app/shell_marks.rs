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
    ///
    /// Ghost 联想顺带在这里维护：`B` 定输入起点，`C` 到达时把刚提交的
    /// 命令读进建议来源（没有 `C` 的 shell 不进建议来源，只是不联想，
    /// 不影响终端工作）；`A` / `D` 使旧锚点作废。
    pub(crate) fn apply_shell_mark(&mut self, mark: ShellMark) {
        match mark.mark {
            crate::domain::shell_marks::Osc133Mark::PromptStart => {
                self.ghost.on_prompt_start();
            }
            crate::domain::shell_marks::Osc133Mark::CommandInput => {
                let column = mark.column.max(0) as usize;
                self.ghost.on_input_start(mark.line, column);
            }
            crate::domain::shell_marks::Osc133Mark::PreExec => {
                self.capture_pending_command();
                self.ghost.on_preexec();
            }
            crate::domain::shell_marks::Osc133Mark::CommandEnd { .. } => {
                self.ghost.on_command_end();
            }
        }
        self.shell_marks.apply(mark);
    }

    /// `C` 标记到达时，把待定命令的输入行从网格读出来记进建议来源。
    /// 读的是待定命令自己的起点行与提示符末尾列，行里只有提示符 +
    /// 命令本身；行已滚出滚动区或起点未知时跳过，不写空命令。
    /// 侧栏的「当前命令」顺带记进命令记录：ghost 只做联想，
    /// 命令记录才是侧栏状态的来源。
    fn capture_pending_command(&mut self) {
        let Some((line, column)) = self.shell_marks.pending_input() else {
            return;
        };
        let Some(text) = self.pty().and_then(|pty| pty.line_text(line, column)) else {
            return;
        };
        self.shell_marks.note_pending_text(&text);
        self.ghost.record(&text);
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
