// @Time    : 2026/11/11
// @Author  : fzf
// @FileName: ghost.rs
// @Software: Siskin

//! 行内自动补全（ghost text）的纯逻辑：建议选取、后缀计算、词边界、
//! 宽字符列宽。只依赖输入串与本会话命令记录，不碰 PTY、文件与网络。
//!
//! 联想来源只有本会话里已执行的命令（`record`，由应用层在
//! OSC 133 `C` 标记到达时从网格读入），不读 `~/.bash_history`，
//! 不联网，不调外部 AI。终端输出按不可信输入处理：建议只画出来，
//! 接受前不向 shell 发任何字节。

use std::collections::VecDeque;

/// 每会话保留的已执行命令上限。去重后保存，超了丢最老的，内存有界。
pub(crate) const MAX_GHOST_COMMANDS: usize = 200;

/// 已执行命令的记录：最新的在前，重复命令提到最前，不重复存。
#[derive(Clone, Debug, Default)]
pub(crate) struct SuggestionHistory {
    commands: VecDeque<String>,
}

impl SuggestionHistory {
    /// 记一条已执行命令。空串与纯空白不记；已有的提到最前，不重复存。
    pub(crate) fn record(&mut self, command: &str) {
        let command = command.trim();
        if command.is_empty() {
            return;
        }
        self.commands.retain(|item| item != command);
        self.commands.push_front(command.to_string());
        while self.commands.len() > MAX_GHOST_COMMANDS {
            self.commands.pop_back();
        }
    }

    /// 取最近一条以 `input` 开头的命令，返回它超出 `input` 的后缀。
    /// 空输入、完全相等（没有后缀）时返回 `None`。
    pub(crate) fn suggest(&self, input: &str) -> Option<String> {
        if input.is_empty() {
            return None;
        }
        self.commands
            .iter()
            .find(|command| command.len() > input.len() && command.starts_with(input))
            .map(|command| command[input.len()..].to_string())
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.commands.len()
    }

    #[cfg(test)]
    pub(crate) fn first(&self) -> Option<&str> {
        self.commands.front().map(String::as_str)
    }
}

/// 提示符输入起点：OSC 133 `B` 标记到达时刻的稳定行与光标列。
/// 列是单元格列（宽字符占两列），与 `Frame` 的列坐标一致。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PromptAnchor {
    pub line: i32,
    pub column: usize,
}

/// 每会话的 ghost 状态：建议来源、当前提示符锚点、是否正在输入、
// Esc 取消后不再对同一输入联想。
#[derive(Clone, Debug, Default)]
pub(crate) struct GhostState {
    pub history: SuggestionHistory,
    prompt: Option<PromptAnchor>,
    accepting: bool,
    dismissed_for: Option<String>,
}

impl GhostState {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// 新提示符：旧锚点作废，输入尚未开始，取消记录也清掉。
    pub(crate) fn on_prompt_start(&mut self) {
        self.prompt = None;
        self.accepting = false;
        self.dismissed_for = None;
    }

    /// `B` 标记：输入开始，锚点是提示符末尾。
    pub(crate) fn on_input_start(&mut self, line: i32, column: usize) {
        self.prompt = Some(PromptAnchor { line, column });
        self.accepting = true;
        self.dismissed_for = None;
    }

    /// `C` 标记：命令已提交，不再是输入态。
    pub(crate) fn on_preexec(&mut self) {
        self.accepting = false;
    }

    /// `D` 标记：命令结束，锚点已是旧提示符，直接丢掉。
    pub(crate) fn on_command_end(&mut self) {
        self.accepting = false;
        self.prompt = None;
    }

    pub(crate) fn record(&mut self, command: &str) {
        self.history.record(command);
    }

    /// Esc 取消：对当前输入不再联想，直到输入变化。
    pub(crate) fn dismiss(&mut self, input: &str) {
        self.dismissed_for = Some(input.to_string());
    }

    /// 当前输入是否允许联想：正在输入、锚点在、输入非空、没被 Esc 取消过。
    pub(crate) fn allows(&self, input: &str) -> bool {
        self.accepting
            && self.prompt.is_some()
            && !input.is_empty()
            && self.dismissed_for.as_deref() != Some(input)
    }

    /// 给定当前输入，返回联想后缀；不允许或无匹配时 `None`。
    pub(crate) fn suggest(&self, input: &str) -> Option<String> {
        if !self.allows(input) {
            return None;
        }
        self.history.suggest(input)
    }

    pub(crate) fn prompt(&self) -> Option<PromptAnchor> {
        self.prompt
    }
}

/// 右方向键接受一个词（fish / zsh-autosuggestions 惯例）：
/// 前导空白 + 一个非空白词，不含词后的空白。输入是 ghost 后缀
///（当前输入之后的剩余部分），返回它开头的一词。
pub(crate) fn first_word(text: &str) -> &str {
    let mut end = 0usize;
    let mut in_word = false;
    for (index, ch) in text.char_indices() {
        if ch.is_whitespace() {
            if in_word {
                break;
            }
        } else {
            in_word = true;
        }
        end = index + ch.len_utf8();
    }
    &text[..end]
}

/// 一个字符占几列终端格。组合标记不占格，宽字符占两格，其余一格。
/// 只用来截断 ghost 文本与逐格定位，精度与网格的 spacer 判定一致即可。
pub(crate) fn char_width(ch: char) -> usize {
    let cp = ch as u32;
    if (0x0300..=0x036f).contains(&cp)
        || (0x1ab0..=0x1aff).contains(&cp)
        || (0x1dc0..=0x1dff).contains(&cp)
        || (0x20d0..=0x20ff).contains(&cp)
        || (0xfe00..=0xfe0f).contains(&cp)
        || (0xfe20..=0xfe2f).contains(&cp)
    {
        return 0;
    }
    if (0x1100..=0x115f).contains(&cp)
        || (0x2e80..=0x303e).contains(&cp)
        || (0x3041..=0x33ff).contains(&cp)
        || (0x3400..=0x4dbf).contains(&cp)
        || (0x4e00..=0x9fff).contains(&cp)
        || (0xa000..=0xa4cf).contains(&cp)
        || (0xac00..=0xd7a3).contains(&cp)
        || (0xf900..=0xfaff).contains(&cp)
        || (0xfe30..=0xfe4f).contains(&cp)
        || (0xff00..=0xff60).contains(&cp)
        || (0xffe0..=0xffe6).contains(&cp)
        || (0x1f300..=0x1f64f).contains(&cp)
        || (0x1f900..=0x1f9ff).contains(&cp)
        || (0x20000..=0x2fffd).contains(&cp)
        || (0x30000..=0x3fffd).contains(&cp)
    {
        return 2;
    }
    1
}

/// 把文本截到 `max_columns` 列以内，只在字符边界处断开。
/// 组合标记跟在基字后面，不会因为基字正好到边而被丢下。
pub(crate) fn truncate_to_columns(text: &str, max_columns: usize) -> &str {
    let mut columns = 0usize;
    for (index, ch) in text.char_indices() {
        let width = char_width(ch);
        if columns + width > max_columns {
            return &text[..index];
        }
        columns += width;
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn history(commands: &[&str]) -> SuggestionHistory {
        let mut history = SuggestionHistory::default();
        for command in commands {
            history.record(command);
        }
        history
    }

    #[test]
    fn suggestion_prefers_the_most_recent_prefix_match() {
        let history = history(&["git status", "git stash", "ls"]);
        assert_eq!(history.suggest("git "), Some("stash".to_string()));
        assert_eq!(history.suggest("git st"), Some("ash".to_string()));
        assert_eq!(history.suggest("ls"), None, "完全相等没有后缀可画");
        assert_eq!(history.suggest(""), None, "空输入不联想");
        assert_eq!(history.suggest("zzz"), None, "无匹配不联想");
    }

    #[test]
    fn suggestion_is_a_prefix_match_not_a_substring_match() {
        let history = history(&["git status"]);
        assert_eq!(history.suggest("sta"), None, "中间子串不算匹配");
        assert_eq!(history.suggest("GIT "), None, "命令区分大小写");
    }

    #[test]
    fn recording_deduplicates_and_moves_to_front() {
        let mut history = history(&["git status", "ls"]);
        history.record("git status");
        assert_eq!(history.len(), 2);
        assert_eq!(history.first(), Some("git status"));
        assert_eq!(history.suggest("g"), Some("it status".to_string()));
        history.record("  ");
        assert_eq!(history.len(), 2, "纯空白不进记录");
        history.record("");
        assert_eq!(history.len(), 2, "空串不进记录");
    }

    #[test]
    fn history_is_bounded_and_drops_the_oldest() {
        let mut history = SuggestionHistory::default();
        for index in 0..=MAX_GHOST_COMMANDS {
            history.record(&format!("cmd {index}"));
        }
        assert_eq!(history.len(), MAX_GHOST_COMMANDS);
        assert_eq!(history.suggest("cmd 0"), None, "最老的已被丢弃");
        assert_eq!(
            history.first(),
            Some(format!("cmd {MAX_GHOST_COMMANDS}").as_str())
        );
    }

    #[test]
    fn first_word_takes_leading_space_plus_one_word() {
        assert_eq!(first_word(" commit -m"), " commit");
        assert_eq!(first_word("status"), "status");
        assert_eq!(first_word(" -m \"x\""), " -m");
        assert_eq!(first_word("   "), "   ", "全是空白就整段接受");
        assert_eq!(first_word(""), "");
        assert_eq!(first_word("中文 测试"), "中文");
    }

    #[test]
    fn char_width_matches_the_grid_spacer_rules() {
        assert_eq!(char_width('a'), 1);
        assert_eq!(char_width('中'), 2, "CJK 占两格");
        assert_eq!(char_width('😀'), 2, "Emoji 占两格");
        assert_eq!(char_width('\u{e0b0}'), 1, "私用区图标占一格");
        assert_eq!(char_width('\u{301}'), 0, "组合重音不占格");
    }

    #[test]
    fn truncation_stops_at_column_boundaries() {
        assert_eq!(truncate_to_columns("abcdef", 3), "abc");
        assert_eq!(truncate_to_columns("中文ab", 3), "中", "宽字符不能切一半");
        assert_eq!(truncate_to_columns("中文ab", 4), "中文");
        assert_eq!(
            truncate_to_columns("e\u{301}x", 1),
            "e\u{301}",
            "组合标记跟基字走"
        );
        assert_eq!(truncate_to_columns("ab", 10), "ab");
        assert_eq!(truncate_to_columns("", 0), "");
    }

    #[test]
    fn ghost_state_follows_the_prompt_marks() {
        let mut state = GhostState::new();
        state.record("git status");
        assert_eq!(state.suggest("git "), None, "没收到 B 之前不联想");
        state.on_input_start(5, 2);
        assert_eq!(state.suggest("git "), Some("status".to_string()));
        state.on_preexec();
        assert_eq!(state.suggest("git "), None, "命令提交后不再联想");
        state.on_input_start(7, 2);
        assert_eq!(state.suggest("git "), Some("status".to_string()));
        state.dismiss("git ");
        assert_eq!(state.suggest("git "), None, "Esc 取消后同一输入不再联想");
        assert_eq!(
            state.suggest("git s"),
            Some("tatus".to_string()),
            "输入变了就恢复"
        );
        state.on_command_end();
        assert_eq!(state.suggest("git s"), None, "命令结束锚点作废");
        state.on_prompt_start();
        state.on_input_start(9, 0);
        assert_eq!(
            state.suggest("git "),
            Some("status".to_string()),
            "新提示符恢复联想"
        );
    }
}
