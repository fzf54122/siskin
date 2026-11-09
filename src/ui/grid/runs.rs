// @Time    : 2026/11/5 19:20
// @Author  : fzf
// @FileName: runs.rs
// @Software: Siskin

//! 把一行 cell 聚成同一样式 run。合并相邻同样式单元格能把整行送进一次
//! `shape_line`，比逐 cell 整形快一个量级，也天然支持连字。

use crate::io::vt::{CellStyle, RowView};

/// 一行里样式一致的连续 cell。
pub(crate) struct Run {
    pub start_column: usize,
    pub cell_count: usize,
    pub text: String,
    pub style: CellStyle,
}

impl Run {
    /// Nerd Font 图标和 emoji 的自然推进量往往不等于一个单元格宽度。
    /// 这类字形必须按列单独定位，否则后面的文字会跟着漂移。
    pub(crate) fn needs_cell_positioning(&self) -> bool {
        !self.text.is_ascii()
    }
}

pub(crate) fn runs_for_row(line: &RowView) -> Vec<Run> {
    let mut runs: Vec<Run> = Vec::new();
    for (column, cell) in line.cells.iter().enumerate() {
        if cell.spacer {
            // 宽字符的后半格属于前一个字：宽度算进去，文本不重复。
            if let Some(run) = runs.last_mut() {
                run.cell_count += 1;
            }
            continue;
        }
        let style = cell.style;
        let mergeable = runs
            .last()
            .is_some_and(|run| run.style == style && run.start_column + run.cell_count == column);
        if mergeable {
            if let Some(run) = runs.last_mut() {
                run.text.push_str(&cell.text);
                run.cell_count += 1;
            }
        } else {
            runs.push(Run {
                start_column: column,
                cell_count: 1,
                text: cell.text.clone(),
                style,
            });
        }
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::vt::{CellView, RowView};

    fn cell(text: &str) -> CellView {
        CellView {
            text: text.to_string(),
            style: CellStyle {
                fg: 0xffffff,
                ..CellStyle::default()
            },
            spacer: false,
            selected: false,
        }
    }

    fn row(cells: Vec<CellView>) -> RowView {
        RowView { cells }
    }

    #[test]
    fn equal_styles_merge_into_one_run() {
        let runs = runs_for_row(&row(vec![cell("a"), cell("b"), cell("c")]));
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].text, "abc");
        assert_eq!(runs[0].cell_count, 3);
    }

    #[test]
    fn style_changes_start_a_new_run() {
        let mut second = cell("b");
        second.style.bold = true;
        assert_eq!(runs_for_row(&row(vec![cell("a"), second])).len(), 2);
    }

    #[test]
    fn wide_char_spacers_count_columns_without_duplicating_text() {
        let mut spacer = cell("");
        spacer.spacer = true;
        let runs = runs_for_row(&row(vec![cell("中"), spacer, cell("文")]));
        assert_eq!(runs.len(), 1);
        // 三列：中占两列（含它的 spacer），文占第三列。
        assert_eq!(runs[0].cell_count, 3);
        assert_eq!(runs[0].text, "中文");
    }

    #[test]
    fn non_ascii_runs_need_cell_positioning() {
        assert!(!runs_for_row(&row(vec![cell("a"), cell("b")]))[0].needs_cell_positioning());
        assert!(runs_for_row(&row(vec![cell("中")]))[0].needs_cell_positioning());
        assert!(runs_for_row(&row(vec![cell("\u{e0b0}")]))[0].needs_cell_positioning());
    }
}
