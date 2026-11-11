// @Time    : 2026/11/5 22:30
// @Author  : fzf
// @FileName: search_ops.rs
// @Software: Siskin

//! 搜索条状态：查询词、选项、当前匹配下标与历史匹配结果，渲染只读。
//! 匹配是绝对行号（`HistorySpan`，负数为历史行），渲染时再按
//! `Frame.display_offset` 换算；查询词、选项与帧快照 `Arc` 指针
//! 都没变时直接复用，不重复扫 10k 行。

use std::sync::Arc;

use crate::io::history_search::HistorySpan;
use crate::io::search::{MatchSpan, next_index};
use crate::io::vt::Frame;

#[derive(Clone, Debug, Default)]
pub(crate) struct SearchState {
    pub query: String,
    pub matches: Vec<HistorySpan>,
    pub current: Option<usize>,
    pub case_sensitive: bool,
    pub regex: bool,
    /// 无效正则的构建错误文案。`None` 表示上次搜索合法（或尚未搜过）。
    pub error: Option<String>,
    cached_query: String,
    cached_case_sensitive: bool,
    cached_regex: bool,
    /// 上次扫描时的帧快照：`Arc` 指针相同即内容没变（新输出 / 滚动 /
    /// resize 都会换一个 `Arc`），可直接复用匹配结果。
    cached_frame: Option<Arc<Frame>>,
}

impl SearchState {
    pub(crate) fn type_char(&mut self, ch: char) {
        if ch.is_control() {
            return;
        }
        self.query.push(ch);
    }

    pub(crate) fn backspace(&mut self) {
        self.query.pop();
    }

    pub(crate) fn toggle_case(&mut self) {
        self.case_sensitive = !self.case_sensitive;
    }

    pub(crate) fn toggle_regex(&mut self) {
        self.regex = !self.regex;
    }

    /// 查询词、选项或帧快照任一变化时才需要重扫；都相同则复用。
    /// 空查询词没有匹配可扫（由 `refresh_history` 清空），返回 `false`。
    pub(crate) fn needs_rescan(&self, frame: &Arc<Frame>) -> bool {
        if self.query.is_empty() {
            return false;
        }
        let same_frame = self
            .cached_frame
            .as_ref()
            .is_some_and(|cached| Arc::ptr_eq(cached, frame));
        !same_frame
            || self.cached_query != self.query
            || self.cached_case_sensitive != self.case_sensitive
            || self.cached_regex != self.regex
    }

    /// 需要重扫时才调 `scan`（通常是 `Pty::search_history`），否则复用。
    pub(crate) fn refresh_history<F>(&mut self, frame: Arc<Frame>, scan: F)
    where
        F: FnOnce() -> Result<Vec<HistorySpan>, String>,
    {
        if self.query.is_empty() {
            self.matches = Vec::new();
            self.current = None;
            self.error = None;
            self.remember(frame);
            return;
        }
        if !self.needs_rescan(&frame) {
            return;
        }
        let result = scan();
        self.apply_history(frame, result);
    }

    /// 存入一次扫描结果并更新缓存键。无效正则时清空匹配、记下错误，
    /// 查询词与开关保持不动。
    pub(crate) fn apply_history(
        &mut self,
        frame: Arc<Frame>,
        result: Result<Vec<HistorySpan>, String>,
    ) {
        match result {
            Ok(matches) => {
                self.matches = matches;
                self.error = None;
            }
            Err(error) => {
                self.matches = Vec::new();
                self.error = Some(error);
            }
        }
        self.current = next_index(None, self.matches.len(), true);
        self.remember(frame);
    }

    /// 滚动只换 `display_offset`，不改匹配的绝对行：同步缓存里的帧
    /// 指针，下次刷新不用为一次滚动白扫一遍历史。
    pub(crate) fn sync_frame(&mut self, frame: Arc<Frame>) {
        if self.cached_query == self.query
            && self.cached_case_sensitive == self.case_sensitive
            && self.cached_regex == self.regex
        {
            self.cached_frame = Some(frame);
        }
    }

    fn remember(&mut self, frame: Arc<Frame>) {
        self.cached_query = self.query.clone();
        self.cached_case_sensitive = self.case_sensitive;
        self.cached_regex = self.regex;
        self.cached_frame = Some(frame);
    }

    pub(crate) fn step(&mut self, forward: bool) {
        self.current = next_index(self.current, self.matches.len(), forward);
    }

    pub(crate) fn current_span(&self) -> Option<HistorySpan> {
        self.current
            .and_then(|index| self.matches.get(index))
            .copied()
    }

    /// 当前帧里可见的全部匹配（绝对行换算成可见行），供网格高亮。
    pub(crate) fn visible_spans(&self, display_offset: usize, rows: usize) -> Vec<MatchSpan> {
        self.matches
            .iter()
            .filter_map(|span| span.visible(display_offset, rows))
            .collect()
    }

    /// 当前匹配在可见帧里的区间，不可见时为 `None`（步进前先滚动）。
    pub(crate) fn current_visible_span(
        &self,
        display_offset: usize,
        rows: usize,
    ) -> Option<MatchSpan> {
        self.current_span()
            .and_then(|span| span.visible(display_offset, rows))
    }

    pub(crate) fn label(&self) -> String {
        let case = if self.case_sensitive { " Aa" } else { "" };
        let regex = if self.regex { " .*" } else { "" };
        let flags = format!("{case}{regex}");
        if let Some(error) = &self.error {
            let first = error.lines().next().unwrap_or("正则错误");
            let short: String = first.chars().take(60).collect();
            return format!("{} · 正则错误：{short}{flags}", self.query);
        }
        if self.query.is_empty() {
            return format!("搜索{flags}");
        }
        match self.matches.len() {
            0 => format!("{} · 无匹配{flags}", self.query),
            total => {
                let index = self.current.map_or(0, |index| index + 1);
                format!("{} · {index}/{total}{flags}", self.query)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::vt::{CellStyle, CellView, Frame, RowView};

    fn frame(rows: &[&str]) -> Frame {
        Frame {
            rows: rows
                .iter()
                .map(|text| RowView {
                    cells: text
                        .chars()
                        .map(|ch| CellView {
                            text: ch.to_string(),
                            style: CellStyle::default(),
                            spacer: false,
                            selected: false,
                            link: None,
                        })
                        .collect(),
                })
                .collect(),
            ..Frame::empty(0)
        }
    }

    /// 测试里没有 PTY，用可见帧的行文本冒充一次历史扫描：
    /// 帧偏移为 0，可见行号即绝对行号。测试行均为 ASCII，列号按字符数算。
    fn scan_from(frame: &Frame, query: &str) -> Result<Vec<HistorySpan>, String> {
        let mut spans = Vec::new();
        for (row, line) in frame.rows.iter().enumerate() {
            let text: String = line.cells.iter().map(|cell| cell.text.as_str()).collect();
            let mut start = 0;
            while let Some(at) = text[start..].find(query) {
                let at = start + at;
                spans.push(HistorySpan {
                    line: row as i32,
                    start: at,
                    end: at + query.chars().count(),
                });
                start = at + query.len();
            }
        }
        Ok(spans)
    }

    fn refresh(state: &mut SearchState, snapshot: &Arc<Frame>) {
        let query = state.query.clone();
        let frame = Arc::clone(snapshot);
        state.refresh_history(Arc::clone(snapshot), move || scan_from(&frame, &query));
    }

    #[test]
    fn typing_recomputes_matches_and_selects_the_first() {
        let mut state = SearchState::default();
        let snapshot = Arc::new(frame(&["error one", "two error", "error three"]));
        for ch in "error".chars() {
            state.type_char(ch);
            refresh(&mut state, &snapshot);
        }
        assert_eq!(state.matches.len(), 3);
        assert_eq!(state.current, Some(0));
        assert_eq!(state.current_span(), Some(state.matches[0]));
        assert_eq!(state.label(), "error · 1/3");
    }

    #[test]
    fn stepping_wraps_both_ways() {
        let mut state = SearchState::default();
        let snapshot = Arc::new(frame(&["x", "x"]));
        state.type_char('x');
        refresh(&mut state, &snapshot);
        assert_eq!(state.current, Some(0));
        state.step(true);
        assert_eq!(state.current, Some(1));
        state.step(true);
        assert_eq!(state.current, Some(0));
        state.step(false);
        assert_eq!(state.current, Some(1));
    }

    #[test]
    fn backspace_drops_the_last_character_and_resets_selection() {
        let mut state = SearchState::default();
        let snapshot = Arc::new(frame(&["a"]));
        state.type_char('a');
        state.type_char('b');
        state.backspace();
        assert_eq!(state.query, "a");
        refresh(&mut state, &snapshot);
        assert_eq!(state.current, Some(0));
    }

    #[test]
    fn an_empty_query_has_no_matches() {
        let mut state = SearchState::default();
        let snapshot = Arc::new(frame(&["anything"]));
        state.refresh_history(Arc::clone(&snapshot), || Ok(Vec::new()));
        assert!(state.matches.is_empty());
        assert_eq!(state.current, None);
        assert_eq!(state.label(), "搜索");
    }

    #[test]
    fn the_same_snapshot_is_not_scanned_twice() {
        use std::cell::Cell;
        fn rescan(state: &mut SearchState, snapshot: &Arc<Frame>, calls: &Cell<u32>) {
            let frame = frame(&["x", "x"]);
            state.refresh_history(Arc::clone(snapshot), move || {
                calls.set(calls.get() + 1);
                scan_from(&frame, "x")
            });
        }
        let mut state = SearchState::default();
        let snapshot = Arc::new(frame(&["x", "x"]));
        state.type_char('x');
        let calls = Cell::new(0);
        rescan(&mut state, &snapshot, &calls);
        assert_eq!(calls.get(), 1, "首轮必须扫一次");
        assert_eq!(state.matches.len(), 2);
        // 同一帧、同一查询与选项：二次刷新直接复用，不再调扫描。
        rescan(&mut state, &snapshot, &calls);
        assert_eq!(calls.get(), 1, "同帧二次刷新不该重扫");
        assert_eq!(state.matches.len(), 2);
        // 换一个内容相同但指针不同的帧：指针变了就必须重扫。
        let other = Arc::new(frame(&["x", "x"]));
        rescan(&mut state, &other, &calls);
        assert_eq!(calls.get(), 2, "帧指针变化时必须重扫");
        // 只改开关也必须重扫（缓存键含选项）。
        state.toggle_case();
        rescan(&mut state, &other, &calls);
        assert_eq!(calls.get(), 3, "大小写开关变化时必须重扫");
    }

    #[test]
    fn an_invalid_regex_keeps_the_query_and_shows_the_error() {
        let mut state = SearchState::default();
        let snapshot = Arc::new(frame(&["anything"]));
        for ch in "[bad".chars() {
            state.type_char(ch);
        }
        state.apply_history(
            Arc::clone(&snapshot),
            Err("regex parse error: unclosed character class".to_string()),
        );
        assert!(state.matches.is_empty());
        assert_eq!(state.current, None);
        assert_eq!(state.query, "[bad", "错误不能清空用户的查询词");
        assert!(state.label().contains("正则错误"), "label 必须露出错误");
        assert!(state.label().contains("[bad"));
    }

    #[test]
    fn visible_spans_translate_absolute_lines_with_the_offset() {
        let mut state = SearchState {
            matches: vec![
                HistorySpan {
                    line: -2,
                    start: 0,
                    end: 3,
                },
                HistorySpan {
                    line: 0,
                    start: 4,
                    end: 7,
                },
                HistorySpan {
                    line: 5,
                    start: 0,
                    end: 1,
                },
            ],
            current: Some(1),
            ..Default::default()
        };
        let shown = state.visible_spans(2, 4);
        assert_eq!(shown.len(), 2, "偏移 2 时 -2 与 0 行可见，5 行在帧外");
        assert_eq!((shown[0].row, shown[0].start, shown[0].end), (0, 0, 3));
        assert_eq!(shown[1].row, 2);
        let current = state.current_visible_span(2, 4).expect("当前匹配应可见");
        assert_eq!((current.row, current.start, current.end), (2, 4, 7));
        let top = state
            .current_visible_span(0, 4)
            .expect("0 行在偏移 0 时可见");
        assert_eq!((top.row, top.start, top.end), (0, 4, 7));
        state.current = Some(0);
        assert_eq!(
            state.current_visible_span(0, 4),
            None,
            "历史行 -2 在偏移 0 时在帧外"
        );
    }

    #[test]
    fn label_shows_the_option_flags() {
        let mut state = SearchState::default();
        state.toggle_case();
        state.toggle_regex();
        assert_eq!(state.label(), "搜索 Aa .*");
        state.type_char('x');
        let snapshot = Arc::new(frame(&["x"]));
        refresh(&mut state, &snapshot);
        assert_eq!(state.label(), "x · 1/1 Aa .*");
    }
}
