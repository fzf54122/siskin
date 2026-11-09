// @Time    : 2026/11/5 19:20
// @Author  : fzf
// @FileName: runs.rs
// @Software: Siskin

//! 把一行 cell 聚成同一样式 run。合并相邻同样式单元格能把整行送进一次
//! `shape_line`，比逐 cell 整形快一个量级，也天然支持连字。

use std::sync::Arc;

use crate::io::vt::{CellStyle, RowView};

/// 一行里样式一致的连续 cell。
pub(crate) struct Run {
    pub start_column: usize,
    pub cell_count: usize,
    pub text: String,
    pub style: CellStyle,
    /// OSC 8 链接地址。同为一段链接的 cell 才会合并，点击时按首列定位。
    pub link: Option<Arc<str>>,
}

impl Run {
    /// 链接段总是单独成 run，不与非链接内容合并，下划线才能画得干净。
    pub(crate) fn is_link(&self) -> bool {
        self.link.is_some()
    }

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
        let mergeable = runs.last().is_some_and(|run| {
            run.style == style
                && run.link == cell.link
                && run.start_column + run.cell_count == column
        });
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
                link: cell.link.clone(),
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
            link: None,
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

    fn linked_cell(text: &str, uri: &str) -> CellView {
        CellView {
            link: Some(std::sync::Arc::from(uri)),
            ..cell(text)
        }
    }

    #[test]
    fn one_hyperlink_becomes_one_linked_run() {
        let runs = runs_for_row(&row(vec![
            cell("a"),
            linked_cell("b", "https://example.com"),
        ]));
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[1].start_column, 1);
        assert_eq!(runs[1].text, "b");
        assert_eq!(runs[1].link.as_deref(), Some("https://example.com"));
        assert!(runs[1].is_link());
    }

    #[test]
    fn a_split_hyperlink_stays_split_so_clicks_land_on_the_right_uri() {
        // 同一行里两段不同地址的链接，中间夹普通文字。
        let runs = runs_for_row(&row(vec![
            linked_cell("x", "https://a.test"),
            cell("-"),
            linked_cell("y", "https://b.test"),
        ]));
        assert_eq!(runs.len(), 3);
        assert_eq!(runs[0].link.as_deref(), Some("https://a.test"));
        assert_eq!(runs[2].link.as_deref(), Some("https://b.test"));
    }

    #[test]
    fn adjacent_cells_of_the_same_uri_merge() {
        let uri = "https://example.com";
        let runs = runs_for_row(&row(vec![
            linked_cell("l", uri),
            linked_cell("i", uri),
            linked_cell("n", uri),
            linked_cell("k", uri),
        ]));
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].text, "link");
        assert_eq!(runs[0].cell_count, 4);
    }

    #[test]
    fn a_link_ending_returns_to_plain_runs() {
        let uri = "https://example.com";
        let runs = runs_for_row(&row(vec![linked_cell("a", uri), cell("b"), cell("c")]));
        assert_eq!(runs.len(), 2);
        assert!(runs[1].link.is_none());
        assert_eq!(runs[1].text, "bc");
    }
}
