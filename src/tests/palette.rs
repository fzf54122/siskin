// @Time    : 2026/11/5 14:40
// @Author  : fzf
// @FileName: palette.rs
// @Software: Siskin

//! 命令面板的过滤与选中范围。面板只保存状态，执行统一走 `Action`，
//! 所以在应用层之外单独验证它更快。

use crate::app::{CommandPalette, PALETTE_ENTRIES};

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
    assert!(palette.matches(close));
    assert!(!palette.matches(new_session));
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
