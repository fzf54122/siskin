// @Time    : 2026/11/5 22:00
// @Author  : fzf
// @FileName: search.rs
// @Software: Siskin

//! 滚动区搜索。只做大小写不敏感的子串匹配，够用且没有额外依赖；
//! 匹配结果以「可见行 + 列区间」返回，渲染层直接按格子高亮。

use crate::io::vt::Frame;

/// 一个匹配区间。`row` 是可见行下标，列区间左闭右开。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MatchSpan {
    pub row: usize,
    pub start: usize,
    pub end: usize,
}

/// 把一行的 cell 拼成字符串，同时记录每个字符对应的列号。
/// 宽字符的后半格不产生字符，所以列号不能直接用字符下标。
fn row_text(frame: &Frame, row: usize) -> (String, Vec<usize>) {
    let mut text = String::new();
    let mut columns = Vec::new();
    let Some(line) = frame.rows.get(row) else {
        return (text, columns);
    };
    for (column, cell) in line.cells.iter().enumerate() {
        if cell.spacer {
            continue;
        }
        for ch in cell.text.chars() {
            text.push(ch);
            columns.push(column);
        }
    }
    (text, columns)
}

/// 在当前帧里找全部匹配。返回顺序按行、列递增，便于「下一个」。
pub(crate) fn find(frame: &Frame, needle: &str) -> Vec<MatchSpan> {
    if needle.is_empty() {
        return Vec::new();
    }
    let needle_chars: Vec<char> = needle.to_lowercase().chars().collect();
    let mut spans = Vec::new();
    for row in 0..frame.rows.len() {
        let (text, columns) = row_text(frame, row);
        if text.is_empty() {
            continue;
        }
        // 按字符比较：`to_lowercase` 可能改变字节长度，用字节下标会错位。
        let haystack: Vec<char> = text.chars().flat_map(char::to_lowercase).collect();
        let mut index = 0;
        while index + needle_chars.len() <= haystack.len() {
            if haystack[index..index + needle_chars.len()] == needle_chars[..] {
                let end = index + needle_chars.len() - 1;
                if let (Some(&start_column), Some(&end_column)) =
                    (columns.get(index), columns.get(end))
                {
                    spans.push(MatchSpan {
                        row,
                        start: start_column,
                        end: end_column + 1,
                    });
                }
                index += needle_chars.len();
            } else {
                index += 1;
            }
        }
    }
    spans
}

/// 「下一个匹配」的循环下标。没有匹配时返回 `None`。
pub(crate) fn next_index(current: Option<usize>, count: usize, forward: bool) -> Option<usize> {
    if count == 0 {
        return None;
    }
    match current {
        None => Some(if forward { 0 } else { count - 1 }),
        Some(index) if forward => Some((index + 1) % count),
        Some(index) => Some((index + count - 1) % count),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::vt::{CellStyle, CellView, Frame, RowView};

    fn row(text: &str) -> RowView {
        RowView {
            cells: text
                .chars()
                .map(|ch| CellView {
                    text: ch.to_string(),
                    style: CellStyle::default(),
                    spacer: false,
                    selected: false,
                })
                .collect(),
        }
    }

    fn frame(rows: &[&str]) -> Frame {
        Frame {
            rows: rows.iter().map(|text| row(text)).collect(),
            ..Frame::empty(0)
        }
    }

    #[test]
    fn finds_every_occurrence_including_case_insensitive_matches() {
        let spans = find(&frame(&["Foo foo", "nothing", "FOO"]), "foo");
        assert_eq!(spans.len(), 3);
        assert_eq!(
            spans[0],
            MatchSpan {
                row: 0,
                start: 0,
                end: 3
            }
        );
        assert_eq!(
            spans[1],
            MatchSpan {
                row: 0,
                start: 4,
                end: 7
            }
        );
        assert_eq!(
            spans[2],
            MatchSpan {
                row: 2,
                start: 0,
                end: 3
            }
        );
    }

    #[test]
    fn an_empty_needle_matches_nothing() {
        assert!(find(&frame(&["anything"]), "").is_empty());
    }

    #[test]
    fn wide_characters_map_back_to_their_columns() {
        // 「中」占两列：本身 + spacer。匹配「文字」应从真实列号开始。
        let mut cells = Vec::new();
        for ch in ["中", "文", "字"] {
            cells.push(CellView {
                text: ch.to_string(),
                style: CellStyle::default(),
                spacer: false,
                selected: false,
            });
            cells.push(CellView {
                text: String::new(),
                style: CellStyle::default(),
                spacer: true,
                selected: false,
            });
        }
        let frame = Frame {
            rows: vec![RowView { cells }],
            ..Frame::empty(0)
        };
        let spans = find(&frame, "文字");
        assert_eq!(spans.len(), 1);
        // 列号：中=0、spacer=1、文=2、spacer=3、字=4、spacer=5。
        assert_eq!(spans[0].start, 2);
        assert_eq!(spans[0].end, 5);
    }

    #[test]
    fn next_index_wraps_in_both_directions() {
        assert_eq!(next_index(None, 3, true), Some(0));
        assert_eq!(next_index(None, 3, false), Some(2));
        assert_eq!(next_index(Some(2), 3, true), Some(0));
        assert_eq!(next_index(Some(0), 3, false), Some(2));
        assert_eq!(next_index(Some(1), 0, true), None);
    }
}
