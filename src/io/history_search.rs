// @Time    : 2026/11/11
// @Author  : fzf
// @FileName: history_search.rs
// @Software: Siskin

//! 完整 scrollback 历史搜索。走 `alacritty_terminal` 的 `RegexSearch` /
//! `Term::search_next` 直接扫网格（含历史行），不拼 10k 行文本。
//! 字面模式先转义正则元字符，大小写用 `(?i)` / `(?-i)` 前缀绕过
//! 其智能大小写；无效正则返回错误文案，由搜索状态显示，不崩溃。

use alacritty_terminal::Term;
use alacritty_terminal::event::EventListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Direction, Line, Point, Side};
use alacritty_terminal::term::search::{Match, RegexSearch};

use crate::io::search::MatchSpan;

/// 全量历史搜索的匹配上限：10k 行里 `a` 这类单字符能命中上万次，
///  cap 住结果集与步进循环，内存与耗时都有界。
pub(crate) const MAX_HISTORY_MATCHES: usize = 1000;

/// 一个历史匹配区间。`line` 是 alacritty 的绝对行号，负数为历史行
///（`Line(-1)` 是可视区上一行），`0..` 为可视区行。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct HistorySpan {
    pub line: i32,
    pub start: usize,
    pub end: usize,
}

impl HistorySpan {
    /// 换算到当前可见帧的行号。`display_offset` 是帧顶相对可视区首行
    /// 的偏移（`Frame.display_offset`），行在可见区外时返回 `None`。
    pub(crate) fn visible(&self, display_offset: usize, rows: usize) -> Option<MatchSpan> {
        let row = self.line + display_offset as i32;
        if row < 0 || row as usize >= rows {
            return None;
        }
        Some(MatchSpan {
            row: row as usize,
            start: self.start,
            end: self.end,
        })
    }
}

/// 把字面查询词转成正则安全的模式：元字符逐个转义。
pub(crate) fn escape_literal(query: &str) -> String {
    let mut escaped = String::with_capacity(query.len());
    for ch in query.chars() {
        if "\\.+*?()|[]{}^$".contains(ch) {
            escaped.push('\\');
        }
        escaped.push(ch);
    }
    escaped
}

/// 拼出交给 `RegexSearch` 的模式。大小写开关用行内标志前缀实现：
/// `RegexSearch` 内部只有智能大小写（有大写才敏感），`(?i)` / `(?-i)`
/// 能显式覆盖它；字面模式先转义再加前缀。
pub(crate) fn build_pattern(query: &str, case_sensitive: bool, regex: bool) -> String {
    let body = if regex {
        query.to_string()
    } else {
        escape_literal(query)
    };
    let flag = if case_sensitive { "(?-i)" } else { "(?i)" };
    format!("{flag}{body}")
}

/// 只校验模式是否合法（不扫网格），供无 PTY 时也能报无效正则。
pub(crate) fn validate_pattern(
    query: &str,
    case_sensitive: bool,
    regex: bool,
) -> Result<(), String> {
    if query.is_empty() {
        return Ok(());
    }
    let pattern = build_pattern(query, case_sensitive, regex);
    RegexSearch::new(&pattern)
        .map(|_| ())
        .map_err(|error| error.to_string())
}
/// 从最老行开始向右收集，绕回起点或达到 `MAX_HISTORY_MATCHES` 即停。
/// 无效正则返回构建错误文案，调用方存进搜索状态显示，不崩溃。
pub(crate) fn search_history<T>(
    term: &Term<T>,
    query: &str,
    case_sensitive: bool,
    regex: bool,
) -> Result<Vec<HistorySpan>, String>
where
    T: EventListener,
{
    if query.is_empty() {
        return Ok(Vec::new());
    }
    let pattern = build_pattern(query, case_sensitive, regex);
    let mut search = RegexSearch::new(&pattern).map_err(|error| error.to_string())?;
    let columns = term.columns();
    let topmost = Line(-(term.history_size() as i32));
    let bottommost = Line(term.screen_lines() as i32 - 1);
    let mut spans = Vec::new();
    let mut origin = Point::new(topmost, Column(0));
    let mut first: Option<Match> = None;
    while spans.len() < MAX_HISTORY_MATCHES {
        let Some(found) =
            term.search_next(&mut search, origin, Direction::Right, Side::Right, None)
        else {
            break;
        };
        if first.as_ref() == Some(&found) {
            break;
        }
        if first.is_none() {
            first = Some(found.clone());
        }
        spans.push(history_span(&found, columns));
        origin = advance_past(found.end(), columns, topmost, bottommost);
    }
    Ok(spans)
}

/// `search_next` 返回的区间可能跨行（正则含换行或折行）：高亮只画
/// 起始行，`end` 钳到行尾，起始行本身的列区间保持准确。
fn history_span(found: &Match, columns: usize) -> HistorySpan {
    let start = found.start();
    let end = found.end();
    let end_column = if end.line == start.line {
        end.column.0 + 1
    } else {
        columns
    };
    HistorySpan {
        line: start.line.0,
        start: start.column.0,
        end: end_column,
    }
}

/// 下一个搜索起点：上一个匹配结束处的下一格，行末则换行，
/// 末行末格则绕回最老行（下一轮 `search_next` 会返回首个匹配，
/// 触发绕回判定后停止）。
fn advance_past(end: &Point, columns: usize, topmost: Line, bottommost: Line) -> Point {
    if end.column.0 + 1 < columns {
        Point::new(end.line, Column(end.column.0 + 1))
    } else if end.line < bottommost {
        Point::new(Line(end.line.0 + 1), Column(0))
    } else {
        Point::new(topmost, Column(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::vt::OfflineScreen;

    fn offline_screen(columns: usize, rows: usize, history: usize) -> OfflineScreen {
        use crate::io::vt::{OfflineScreen, ScreenSize};
        let (sender, _receiver) = async_channel::unbounded();
        OfflineScreen::new(ScreenSize::new(columns, rows, history), sender, 0)
    }

    #[test]
    fn literal_mode_escapes_regex_metacharacters() {
        assert_eq!(escape_literal("a.c*"), "a\\.c\\*");
        assert_eq!(escape_literal("[x] (y)"), "\\[x\\] \\(y\\)");
        assert_eq!(build_pattern("a.c", false, false), "(?i)a\\.c");
        assert_eq!(build_pattern("a.c", true, false), "(?-i)a\\.c");
        assert_eq!(build_pattern("a.c", false, true), "(?i)a.c");
    }

    #[test]
    fn history_search_reaches_lines_scrolled_off_screen() {
        let mut screen = offline_screen(20, 4, 100);
        for line in 1..=50 {
            screen.feed(format!("line {line}\r\n").as_bytes());
        }
        let spans =
            search_history(&screen.term, "line 42", false, false).expect("字面查询不应报错");
        assert_eq!(spans.len(), 1, "只应命中第 42 行");
        assert!(spans[0].line < 0, "第 42 行早已滚出可视区，应为历史行");
        // 可视帧里确实没有它，证明不是只搜了当前帧。
        let frame = screen.frame(&crate::io::ansi::AnsiColors::from_palette(
            &crate::theme::PAPER,
        ));
        let visible: String = frame
            .rows
            .iter()
            .flat_map(|row| row.cells.iter())
            .map(|cell| cell.text.as_str())
            .collect();
        assert!(!visible.contains("line 42"), "可视帧不应含第 42 行");
        // 换算回可见行：先滚到该行再换算，行号与列区间一致。
        let offset = (-spans[0].line) as usize;
        let shown = spans[0].visible(offset, 4).expect("滚到顶后应可见");
        assert_eq!(shown.start, 0);
        assert_eq!(shown.end, 7);
    }

    #[test]
    fn case_sensitivity_is_explicit_not_smart_case() {
        let mut screen = offline_screen(20, 4, 10);
        screen.feed(b"Foo foo FOO\n");
        let insensitive =
            search_history(&screen.term, "foo", false, false).expect("字面查询不应报错");
        assert_eq!(insensitive.len(), 3, "不敏感应命中三种大小写");
        let sensitive = search_history(&screen.term, "foo", true, false).expect("字面查询不应报错");
        assert_eq!(sensitive.len(), 1, "敏感只应命中全小写的 foo");
        assert_eq!(sensitive[0].start, 4);
    }

    #[test]
    fn regex_mode_matches_patterns_and_reports_invalid_ones() {
        let mut screen = offline_screen(20, 4, 10);
        screen.feed(b"Foo foo FOO\n");
        let matched = search_history(&screen.term, "f.o", false, true).expect("合法正则不应报错");
        assert_eq!(matched.len(), 3, "不敏感正则 f.o 应命中三处");
        let strict = search_history(&screen.term, "f.o", true, true).expect("合法正则不应报错");
        assert_eq!(strict.len(), 1, "敏感正则 f.o 只应命中 foo");
        let error = search_history(&screen.term, "[invalid", false, true);
        assert!(error.is_err(), "无效正则必须返回错误而不是崩溃");
        assert!(!error.expect_err("上面已断言是 Err").is_empty());
        assert!(validate_pattern("[invalid", false, true).is_err());
        assert!(validate_pattern("a.c", false, true).is_ok());
        assert!(validate_pattern("", false, true).is_ok());
    }

    #[test]
    fn history_search_caps_at_one_thousand_matches() {
        let mut screen = offline_screen(20, 4, 2000);
        for _ in 0..1100 {
            screen.feed(b"a\n");
        }
        let spans = search_history(&screen.term, "a", false, false).expect("字面查询不应报错");
        assert_eq!(spans.len(), MAX_HISTORY_MATCHES, "超量匹配必须截断到上限");
    }
}
