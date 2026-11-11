// @Time    : 2026/11/11
// @Author  : fzf
// @FileName: shell_marks.rs
// @Software: Siskin

//! OSC 133 Shell Integration 的纯数据层。标记只采信结构化参数：退出码
//! 只认 `133;D;<code>` 的参数，绝不从普通文本里猜。
//!
//! 行号是单调稳定的行计数（从会话第一行起算），不是 `HistorySpan` 式
//! 行号：后者会随新输出到来整体下移，存下来就漂移；稳定行在跳转时
//! 用当前 `history_size` 换回绝对行。

use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// 一条已解析的 OSC 133 标记。`D` 的退出码是 `Option`：
/// 有的 shell 只发 `133;D` 不带码，缺码时不猜，记 `None`。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Osc133Mark {
    /// 提示符开始（`133;A`）。
    PromptStart,
    /// 命令输入开始（`133;B`，提示符已打完，用户开始输入）。
    CommandInput,
    /// 命令执行开始（`133;C`，回车已按下，命令开始跑）。
    PreExec,
    /// 命令结束（`133;D[;<exit_code>]`）。
    CommandEnd { exit_code: Option<i32> },
}

/// 带锚定行号的标记事件。行号是单调稳定的行计数（从会话第一行起算）：
/// `HistorySpan` 式行号会随新输出到来整体下移，存下来就漂移，所以读线程
/// 记的是 `history_size + 绝对行`，跳转时再用当前 `history_size` 换回。
/// 绝对行 = 视口光标行 - 到达时刻的 `display_offset`。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ShellMark {
    pub line: i32,
    pub mark: Osc133Mark,
}

impl ShellMark {
    pub(crate) fn new(line: i32, mark: Osc133Mark) -> Self {
        Self { line, mark }
    }
}

/// 一条已完成命令的记录。`failed()` 只看 `exit_code`：`Some(0)` 以外
/// （含缺码的 `None`）都不算成功里的失败——缺码只是不知道，不是失败。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CommandSpan {
    /// 提示符所在行（`A` 标记行）。
    pub start_line: i32,
    /// 输入开始行（`B` 标记行；没收到 `B` 时等于 `start_line`）。
    pub input_line: i32,
    /// 命令结束行（`D` 标记行）。
    pub end_line: i32,
    /// 退出码，只认 `D` 标记参数。
    pub exit_code: Option<i32>,
    pub started_at: Instant,
    pub duration: Option<Duration>,
}

impl CommandSpan {
    /// 失败命令：退出码存在且非 0。缺码（`None`）不算失败。
    /// 暂只供测试断言：失败标记先记在记录里，UI/跳转后续再用。
    #[cfg(test)]
    pub(crate) fn failed(&self) -> bool {
        self.exit_code.is_some_and(|code| code != 0)
    }
}

/// 每会话的命令记录上限：100 条，超了丢最老的，内存有界。
pub(crate) const MAX_COMMAND_SPANS: usize = 100;

/// 每会话独立的 A/B/C/D 状态机。无标记时全空是合法态——shell 没装
/// 集成脚本时终端必须正常工作，不能假定每个 shell 都支持。
#[derive(Debug, Default)]
pub(crate) struct CommandLog {
    spans: VecDeque<CommandSpan>,
    pending: Option<PendingCommand>,
}

#[derive(Debug)]
struct PendingCommand {
    start_line: i32,
    input_line: i32,
    started_at: Instant,
}

impl CommandLog {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// 按序消费一个标记。重复的 `A` 会丢弃上一条没跑完的待定命令
    /// （它没有 `D`，不算一条命令）；`D` 独自到达时也记一条，
    /// 起止行都取 `D` 行，起止时间未知。
    pub(crate) fn apply(&mut self, mark: ShellMark) {
        match mark.mark {
            Osc133Mark::PromptStart => {
                self.pending = Some(PendingCommand {
                    start_line: mark.line,
                    input_line: mark.line,
                    started_at: Instant::now(),
                });
            }
            Osc133Mark::CommandInput => {
                if let Some(pending) = self.pending.as_mut() {
                    pending.input_line = mark.line;
                }
            }
            Osc133Mark::PreExec => {
                if let Some(pending) = self.pending.as_mut() {
                    pending.started_at = Instant::now();
                }
            }
            Osc133Mark::CommandEnd { exit_code } => {
                let span = match self.pending.take() {
                    Some(pending) => CommandSpan {
                        start_line: pending.start_line,
                        input_line: pending.input_line,
                        end_line: mark.line,
                        exit_code,
                        started_at: pending.started_at,
                        duration: Some(pending.started_at.elapsed()),
                    },
                    None => CommandSpan {
                        start_line: mark.line,
                        input_line: mark.line,
                        end_line: mark.line,
                        exit_code,
                        started_at: Instant::now(),
                        duration: None,
                    },
                };
                self.push(span);
            }
        }
    }

    fn push(&mut self, span: CommandSpan) {
        self.spans.push_back(span);
        while self.spans.len() > MAX_COMMAND_SPANS {
            self.spans.pop_front();
        }
    }

    /// 暂只供测试读记录；生产路径的跳转只走 `next_after` / `prev_before`。
    #[cfg(test)]
    pub(crate) fn spans(&self) -> &VecDeque<CommandSpan> {
        &self.spans
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.spans.len()
    }

    #[cfg(test)]
    pub(crate) fn is_empty(&self) -> bool {
        self.spans.is_empty()
    }

    /// 跳转用：比 `line` 更新（行号更大）的第一条命令，即下一个命令。
    pub(crate) fn next_after(&self, line: i32) -> Option<&CommandSpan> {
        self.spans.iter().find(|span| span.start_line > line)
    }

    /// 跳转用：比 `line` 更旧（行号更小）的最后一条命令，即上一个命令。
    pub(crate) fn prev_before(&self, line: i32) -> Option<&CommandSpan> {
        self.spans.iter().rev().find(|span| span.start_line < line)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mark(line: i32, kind: Osc133Mark) -> ShellMark {
        ShellMark::new(line, kind)
    }

    #[test]
    fn a_full_cycle_records_one_span_with_exit_code() {
        let mut log = CommandLog::new();
        log.apply(mark(0, Osc133Mark::PromptStart));
        log.apply(mark(0, Osc133Mark::CommandInput));
        log.apply(mark(1, Osc133Mark::PreExec));
        log.apply(mark(3, Osc133Mark::CommandEnd { exit_code: Some(3) }));
        assert_eq!(log.len(), 1);
        let span = &log.spans()[0];
        assert_eq!(span.start_line, 0);
        assert_eq!(span.input_line, 0);
        assert_eq!(span.end_line, 3);
        assert_eq!(span.exit_code, Some(3));
        assert!(span.failed());
        assert!(span.duration.is_some());
    }

    #[test]
    fn exit_code_zero_is_not_failed_and_missing_code_is_unknown() {
        let mut log = CommandLog::new();
        log.apply(mark(0, Osc133Mark::PromptStart));
        log.apply(mark(1, Osc133Mark::CommandEnd { exit_code: Some(0) }));
        assert!(!log.spans()[0].failed());
        log.apply(mark(2, Osc133Mark::CommandEnd { exit_code: None }));
        assert_eq!(log.spans()[1].exit_code, None);
        assert!(!log.spans()[1].failed(), "缺码不是失败，只是不清楚");
    }

    #[test]
    fn a_lone_d_is_still_recorded_without_a_prompt() {
        let mut log = CommandLog::new();
        log.apply(mark(5, Osc133Mark::CommandEnd { exit_code: Some(1) }));
        assert_eq!(log.len(), 1);
        assert_eq!(log.spans()[0].start_line, 5);
        assert_eq!(log.spans()[0].end_line, 5);
        assert!(log.spans()[0].duration.is_none());
    }

    #[test]
    fn a_second_prompt_drops_the_unfinished_command() {
        let mut log = CommandLog::new();
        log.apply(mark(0, Osc133Mark::PromptStart));
        log.apply(mark(1, Osc133Mark::PromptStart));
        log.apply(mark(2, Osc133Mark::CommandEnd { exit_code: Some(0) }));
        assert_eq!(log.len(), 1);
        assert_eq!(log.spans()[0].start_line, 1);
    }

    #[test]
    fn the_log_is_bounded_and_drops_the_oldest() {
        let mut log = CommandLog::new();
        for line in 0..(MAX_COMMAND_SPANS + 5) as i32 {
            log.apply(mark(line, Osc133Mark::CommandEnd { exit_code: Some(0) }));
        }
        assert_eq!(log.len(), MAX_COMMAND_SPANS);
        assert_eq!(log.spans()[0].start_line, 5);
    }

    #[test]
    fn next_and_previous_pick_the_neighbouring_commands() {
        let mut log = CommandLog::new();
        for line in [-10, -5, 0] {
            log.apply(mark(line, Osc133Mark::PromptStart));
            log.apply(mark(
                line + 1,
                Osc133Mark::CommandEnd { exit_code: Some(0) },
            ));
        }
        assert_eq!(log.next_after(-10).unwrap().start_line, -5);
        assert_eq!(log.next_after(0), None);
        assert_eq!(log.prev_before(0).unwrap().start_line, -5);
        assert_eq!(log.prev_before(-10), None);
    }

    #[test]
    fn an_empty_log_is_a_legal_state() {
        let log = CommandLog::new();
        assert!(log.is_empty());
        assert_eq!(log.next_after(0), None);
        assert_eq!(log.prev_before(0), None);
    }
}
