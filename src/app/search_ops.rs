// @Time    : 2026/11/5 22:30
// @Author  : fzf
// @FileName: search_ops.rs
// @Software: Siskin

//! 搜索条状态。查询词、当前匹配下标与匹配结果都放在这里，
//! 渲染只读，按键链路只调方法。

use crate::io::search::{MatchSpan, find, next_index};

#[derive(Clone, Debug, Default)]
pub(crate) struct SearchState {
    pub query: String,
    pub matches: Vec<MatchSpan>,
    pub current: Option<usize>,
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

    /// 每次查询或帧内容变化后重算匹配。
    pub(crate) fn refresh(&mut self, frame: &crate::io::vt::Frame) {
        self.matches = find(frame, &self.query);
        self.current = next_index(None, self.matches.len(), true);
    }

    pub(crate) fn step(&mut self, forward: bool) {
        self.current = next_index(self.current, self.matches.len(), forward);
    }

    pub(crate) fn current_span(&self) -> Option<MatchSpan> {
        self.current
            .and_then(|index| self.matches.get(index))
            .copied()
    }

    pub(crate) fn label(&self) -> String {
        if self.query.is_empty() {
            return "搜索".to_string();
        }
        match self.matches.len() {
            0 => format!("{} · 无匹配", self.query),
            total => {
                let index = self.current.map_or(0, |index| index + 1);
                format!("{} · {index}/{total}", self.query)
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

    #[test]
    fn typing_recomputes_matches_and_selects_the_first() {
        let mut state = SearchState::default();
        for ch in "error".chars() {
            state.type_char(ch);
            state.refresh(&frame(&["error one", "two error", "error three"]));
        }
        assert_eq!(state.matches.len(), 3);
        assert_eq!(state.current, Some(0));
        assert_eq!(state.current_span().as_ref(), Some(&state.matches[0]));
    }

    #[test]
    fn stepping_wraps_both_ways() {
        let mut state = SearchState::default();
        for ch in "x".chars() {
            state.type_char(ch);
        }
        state.refresh(&frame(&["x", "x"]));
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
        state.type_char('a');
        state.type_char('b');
        state.backspace();
        assert_eq!(state.query, "a");
        state.refresh(&frame(&["a"]));
        assert_eq!(state.current, Some(0));
    }

    #[test]
    fn an_empty_query_has_no_matches() {
        let mut state = SearchState::default();
        state.refresh(&frame(&["anything"]));
        assert!(state.matches.is_empty());
        assert_eq!(state.current, None);
        assert_eq!(state.label(), "搜索");
    }
}
