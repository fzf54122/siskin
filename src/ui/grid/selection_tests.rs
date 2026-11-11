// @Time    : 2026/11/10
// @Author  : fzf
// @FileName: selection_tests.rs
// @Software: Siskin

//! 选区矩形合并的补充测试。`mod.rs` 里的内联测试覆盖基本合并；
//! 这里覆盖 P1 验收点：宽字符后半格不留缺口、跨行不错误合并。
//! 单独成文件是为守住 `mod.rs` 的 400 行门禁。

use super::selection_ranges;
use crate::io::vt::{CellStyle, CellView, RowView};

fn cell(selected: bool) -> CellView {
    CellView {
        text: "x".into(),
        style: CellStyle::default(),
        spacer: false,
        selected,
        link: None,
    }
}

fn spacer(selected: bool) -> CellView {
    CellView {
        text: String::new(),
        style: CellStyle::default(),
        spacer: true,
        selected,
        link: None,
    }
}

fn row(cells: Vec<CellView>) -> RowView {
    RowView { cells }
}

#[test]
fn a_selected_wide_char_covers_its_spacer_without_a_gap() {
    // 宽字符已选、后半格未标选：矩形必须盖住两列，否则只画一半。
    let line = row(vec![cell(true), spacer(false), cell(false)]);
    assert_eq!(selection_ranges(&line), vec![(0, 2)]);
}

#[test]
fn a_selected_wide_char_with_a_marked_spacer_stays_one_range() {
    // 两格都标选：仍是一段，不分裂成两段。
    let line = row(vec![cell(true), spacer(true), cell(false)]);
    assert_eq!(selection_ranges(&line), vec![(0, 2)]);
}

#[test]
fn a_spacer_after_an_unselected_cell_does_not_start_a_range() {
    // 前一格没选：spacer 不能凭空开一段，否则空白处多一块高亮。
    let line = row(vec![cell(false), spacer(false), cell(true)]);
    assert_eq!(selection_ranges(&line), vec![(2, 3)]);
}

#[test]
fn a_wide_char_breaks_a_run_when_only_its_second_half_is_selected() {
    // 只有 spacer 标选（退化输入）：仍成段，不丢高亮、不吞前一格。
    let line = row(vec![cell(false), spacer(true), cell(false)]);
    assert_eq!(selection_ranges(&line), vec![(1, 2)]);
}

#[test]
fn wide_chars_do_not_bridge_a_gap_between_selected_cells() {
    // 选中格、未选宽字符、选中格：三段之间不断开合并，宽字符本身不选。
    let line = row(vec![cell(true), cell(false), spacer(false), cell(true)]);
    assert_eq!(selection_ranges(&line), vec![(0, 1), (3, 4)]);
}

#[test]
fn ranges_are_computed_per_row_and_never_merge_across_rows() {
    // 两行各自全选：每行一段；行尾选区不能与下一行行首拼成一段。
    let first = row(vec![cell(true), cell(true)]);
    let second = row(vec![cell(true), cell(true)]);
    assert_eq!(selection_ranges(&first), vec![(0, 2)]);
    assert_eq!(selection_ranges(&second), vec![(0, 2)]);
    // 上行尾选、下行首不选：仍是独立的两段结果，不跨行。
    let tail = row(vec![cell(false), cell(true)]);
    let head = row(vec![cell(false), cell(false)]);
    assert_eq!(selection_ranges(&tail), vec![(1, 2)]);
    assert!(selection_ranges(&head).is_empty());
}
