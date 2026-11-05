// @Time    : 2026/11/4 19:52
// @Author  : fzf
// @FileName: terminal.rs
// @Software: Siskin

//! Scrollback and cursor. This module owns text and layout-independent state
//! only, so the command engine and the tests can drive it without GPUI.

use crate::line::Line;
#[cfg(test)]
use crate::line::Role;

/// Visible rows kept in memory. Old sessions scroll past this and are dropped.
pub const SCROLLBACK_LIMIT: usize = 2_000;
/// Columns the view can show at the narrowest supported window width.
pub const MIN_COLUMNS: usize = 40;

#[derive(Clone, Debug)]
pub struct Terminal {
    lines: Vec<Line>,
    input: String,
    scroll: usize,
    columns: usize,
}

impl Default for Terminal {
    fn default() -> Self {
        Self::new(MIN_COLUMNS)
    }
}

impl Terminal {
    pub fn new(columns: usize) -> Self {
        Self {
            lines: Vec::new(),
            input: String::new(),
            scroll: 0,
            columns: columns.max(MIN_COLUMNS),
        }
    }

    pub fn lines(&self) -> &[Line] {
        &self.lines
    }

    pub fn input(&self) -> &str {
        &self.input
    }

    pub fn columns(&self) -> usize {
        self.columns
    }

    pub fn scroll(&self) -> usize {
        self.scroll
    }

    #[cfg(test)]
    pub fn is_scrolled(&self) -> bool {
        self.scroll > 0
    }

    pub fn set_columns(&mut self, columns: usize) {
        self.columns = columns.max(MIN_COLUMNS);
    }

    pub fn push_line(&mut self, line: Line) {
        self.lines.push(line);
        if self.lines.len() > SCROLLBACK_LIMIT {
            let overflow = self.lines.len() - SCROLLBACK_LIMIT;
            self.lines.drain(0..overflow);
        }
        self.scroll = 0;
    }

    #[cfg(test)]
    pub fn push_text(&mut self, text: impl Into<String>, role: Role) {
        self.push_line(Line::text(text, role));
    }

    pub fn push_blank(&mut self) {
        self.lines.push(Line::default());
    }

    pub fn type_char(&mut self, ch: char) {
        if ch.is_control() || self.input.chars().count() >= self.columns.saturating_sub(4) {
            return;
        }
        self.input.push(ch);
        self.scroll = 0;
    }

    pub fn backspace(&mut self) {
        self.input.pop();
        self.scroll = 0;
    }

    /// Removes and returns the typed command. History is appended by the caller.
    pub fn take_input(&mut self) -> String {
        std::mem::take(&mut self.input)
    }

    pub fn clear_input(&mut self) {
        self.input.clear();
        self.scroll = 0;
    }

    pub fn scroll_up(&mut self, rows: usize) {
        self.scroll = (self.scroll + rows).min(self.lines.len());
    }

    pub fn scroll_down(&mut self, rows: usize) {
        self.scroll = self.scroll.saturating_sub(rows);
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scroll = 0;
    }

    pub fn clear(&mut self) {
        self.lines.clear();
        self.scroll = 0;
    }

    /// Applies a history entry to the input line, replacing whatever is typed.
    pub fn set_input(&mut self, text: impl Into<String>) {
        self.input = text.into();
        self.input
            .truncate_on_char_boundary(self.columns.saturating_sub(4));
        self.scroll = 0;
    }

    /// Physical rows after wrapping, used for both layout and tests.
    pub fn wrapped(&self) -> Vec<Line> {
        self.lines
            .iter()
            .flat_map(|line| line.wrap(self.columns))
            .collect()
    }
}

trait TruncateOnCharBoundary {
    fn truncate_on_char_boundary(&mut self, max: usize);
}

impl TruncateOnCharBoundary for String {
    fn truncate_on_char_boundary(&mut self, max: usize) {
        if let Some((index, _)) = self.char_indices().nth(max) {
            self.truncate(index);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrollback_is_bounded_and_keeps_the_newest_lines() {
        let mut terminal = Terminal::new(60);
        for index in 0..(SCROLLBACK_LIMIT + 10) {
            terminal.push_text(format!("row {index}"), Role::Output);
        }
        assert_eq!(terminal.lines().len(), SCROLLBACK_LIMIT);
        assert_eq!(
            terminal.lines().last().unwrap().plain(),
            format!("row {}", SCROLLBACK_LIMIT + 9)
        );
    }

    #[test]
    fn typing_respects_the_column_limit_and_rejects_control_keys() {
        let mut terminal = Terminal::new(MIN_COLUMNS);
        terminal.type_char('\t');
        assert!(terminal.input().is_empty());
        for _ in 0..(MIN_COLUMNS * 2) {
            terminal.type_char('x');
        }
        assert_eq!(terminal.input().chars().count(), MIN_COLUMNS - 4);
    }

    #[test]
    fn scrolling_stops_at_the_top_and_snaps_back_on_input() {
        let mut terminal = Terminal::new(40);
        for index in 0..10 {
            terminal.push_text(format!("line {index}"), Role::Output);
        }
        terminal.scroll_up(100);
        assert_eq!(terminal.scroll(), 10);
        assert!(terminal.is_scrolled());
        terminal.type_char('a');
        assert_eq!(terminal.scroll(), 0);
        terminal.scroll_up(2);
        terminal.scroll_down(2);
        assert!(!terminal.is_scrolled());
    }

    #[test]
    fn wrapping_counts_logical_lines_separately() {
        let mut terminal = Terminal::new(MIN_COLUMNS);
        terminal.push_text("x".repeat(MIN_COLUMNS + 10), Role::Output);
        assert_eq!(terminal.wrapped().len(), 2);
        terminal.push_text("second", Role::Output);
        assert_eq!(terminal.wrapped().len(), 3);
    }
}
