// @Time    : 2026/11/5 14:40
// @Author  : fzf
// @FileName: palette.rs
// @Software: Siskin

//! 命令面板的过滤与选中范围。面板只保存状态，执行统一走 `Action`，
//! 所以在应用层之外单独验证它更快。

use crate::app::fuzzy::fuzzy_match;
use crate::app::{CommandPalette, PALETTE_ENTRIES};
use crate::domain::{Action, ShellKind};
use crate::theme::ThemeId;

fn type_query(palette: &mut CommandPalette, query: &str) {
    for ch in query.chars() {
        palette.type_char(ch);
    }
}

#[test]
fn filtering_matches_labels_keywords_and_hints() {
    let mut palette = CommandPalette::default();
    for ch in "关闭".chars() {
        palette.type_char(ch);
    }
    let close = PALETTE_ENTRIES
        .iter()
        .find(|entry| entry.label == "关闭当前会话")
        .expect("面板必须保留关闭会话条目");
    let new_session = PALETTE_ENTRIES
        .iter()
        .find(|entry| entry.label == "新建会话")
        .expect("面板必须保留新建会话条目");
    assert!(palette.score(close).is_some());
    assert!(palette.score(new_session).is_none());
}

#[test]
fn selection_stays_inside_the_visible_range() {
    let mut palette = CommandPalette::default();
    palette.move_selection(true, 3);
    palette.move_selection(true, 3);
    palette.move_selection(true, 3);
    assert_eq!(palette.selected, 2);
    palette.move_selection(false, 3);
    palette.move_selection(false, 3);
    assert_eq!(palette.selected, 0);
    palette.move_selection(true, 0);
    assert_eq!(palette.selected, 0);
}

#[test]
fn fuzzy_matches_subsequences_case_insensitively() {
    assert!(fuzzy_match("nws", "new session tab open").is_some());
    assert!(fuzzy_match("NEW", "new session tab open").is_some());
    assert!(fuzzy_match("zzz", "新建会话").is_none());
    assert!(fuzzy_match("abc", "def").is_none());
    assert!(fuzzy_match("", "new session").is_none());
}

#[test]
fn fuzzy_ranges_point_at_matched_label_chars() {
    let matched = fuzzy_match("关闭", "关闭当前会话").expect("子串必须命中");
    assert_eq!(matched.ranges, vec![(0, 3), (3, 6)]);
}

#[test]
fn fuzzy_prefers_contiguous_and_word_start_matches() {
    let contiguous = fuzzy_match("new", "new session tab open").expect("必须命中");
    let scattered = fuzzy_match("nws", "new session tab open").expect("必须命中");
    assert!(contiguous.score > scattered.score);
    let word_start = fuzzy_match("sp", "split pane").expect("必须命中");
    let mid_word = fuzzy_match("pl", "split pane").expect("必须命中");
    assert!(word_start.score > mid_word.score);
}

#[test]
fn empty_query_ranks_recent_actions_first() {
    let palette = CommandPalette::default();
    let ranked = palette.ranked(&[Action::CloseSession, Action::NewSession]);
    let first_two: Vec<Action> = ranked
        .iter()
        .take(2)
        .map(|(index, _)| PALETTE_ENTRIES[*index].action)
        .collect();
    assert_eq!(first_two, vec![Action::CloseSession, Action::NewSession]);
}

#[test]
fn query_ranks_recent_matches_first() {
    let mut palette = CommandPalette::default();
    type_query(&mut palette, "分屏");
    let ranked = palette.ranked(&[Action::ClosePane]);
    let (first, _) = ranked.first().expect("分屏必须有结果");
    assert_eq!(PALETTE_ENTRIES[*first].action, Action::ClosePane);
    assert!(ranked.iter().all(|(index, _)| {
        PALETTE_ENTRIES[*index].label.contains("分屏")
            || PALETTE_ENTRIES[*index].keywords.contains("split")
    }));
}

#[test]
fn every_action_except_open_palette_has_an_entry() {
    let actions: Vec<Action> = PALETTE_ENTRIES.iter().map(|entry| entry.action).collect();
    for action in [
        Action::NewSession,
        Action::CloseSession,
        Action::Clear,
        Action::Quit,
        Action::OpenSettings,
        Action::OpenTerminal,
        Action::CycleThemeMode,
        Action::FontLarger,
        Action::FontSmaller,
        Action::ToggleInspector,
        Action::NextSession,
        Action::PreviousSession,
        Action::SplitHorizontal,
        Action::SplitVertical,
        Action::FocusNextPane,
        Action::FocusPrevPane,
        Action::ClosePane,
        Action::ScrollPageUp,
        Action::ScrollPageDown,
        Action::ScrollToTop,
        Action::ScrollToBottom,
        Action::NextCommand,
        Action::PreviousCommand,
        Action::OpenSearch,
        Action::TogglePerf,
        Action::ToggleGlass,
        Action::CycleGlassOpacity,
        Action::RenameSession,
        Action::ToggleAgentSidebar,
    ] {
        assert!(actions.contains(&action), "缺少动作条目：{action:?}");
    }
    for kind in ShellKind::ALL {
        assert!(
            actions.contains(&Action::SetShell(kind)),
            "缺少 Shell 条目：{kind:?}"
        );
    }
    // 每套预设（包括新加的 Catppuccin Mocha、Tokyo Night）都能在面板里点选。
    for theme in ThemeId::PRESETS {
        assert!(
            actions.contains(&Action::SetTheme(theme)),
            "缺少主题条目：{}",
            theme.name()
        );
    }
    assert!(!actions.contains(&Action::OpenPalette));
}
