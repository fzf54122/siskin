// @Time    : 2026/11/5 17:42
// @Author  : fzf
// @FileName: text_edit.rs
// @Software: Siskin

use std::ops::Range;

#[derive(Clone, Debug, Default)]
pub(crate) struct TextEdit {
    pub text: String,
    pub cursor: usize,
    pub anchor: usize,
    pub marked: Option<Range<usize>>,
}

impl TextEdit {
    pub(crate) fn new(text: String) -> Self {
        Self {
            cursor: text.len(),
            text,
            ..Self::default()
        }
    }

    pub(crate) fn selection(&self) -> Range<usize> {
        self.anchor.min(self.cursor)..self.anchor.max(self.cursor)
    }

    pub(crate) fn select_all(&mut self) {
        self.anchor = 0;
        self.cursor = self.text.len();
    }

    pub(crate) fn move_to(&mut self, offset: usize, extend: bool) {
        self.cursor = self.boundary(offset);
        if !extend {
            self.anchor = self.cursor;
        }
    }

    pub(crate) fn move_character(&mut self, right: bool, extend: bool) {
        let selection = self.selection();
        let next = if !extend && !selection.is_empty() {
            if right {
                selection.end
            } else {
                selection.start
            }
        } else if right {
            self.text[self.cursor..]
                .chars()
                .next()
                .map_or(self.cursor, |ch| self.cursor + ch.len_utf8())
        } else {
            self.text[..self.cursor]
                .char_indices()
                .next_back()
                .map_or(0, |(offset, _)| offset)
        };
        self.move_to(next, extend);
    }

    pub(crate) fn delete(&mut self, backward: bool) {
        if self.selection().is_empty() {
            let cursor = self.cursor;
            self.move_character(!backward, true);
            self.anchor = cursor;
        }
        self.replace(None, "", false);
    }

    pub(crate) fn replace(&mut self, utf16: Option<Range<usize>>, value: &str, mark: bool) {
        let range = utf16
            .map(|range| self.range_from_utf16(range))
            .or_else(|| self.marked.take())
            .unwrap_or_else(|| self.selection());
        let retained =
            self.text[..range.start].chars().count() + self.text[range.end..].chars().count();
        let value: String = value
            .chars()
            .filter(|ch| !ch.is_control())
            .take(256usize.saturating_sub(retained))
            .collect();
        self.text.replace_range(range.clone(), &value);
        self.cursor = range.start + value.len();
        self.anchor = self.cursor;
        self.marked = (mark && !value.is_empty()).then_some(range.start..self.cursor);
    }

    pub(crate) fn to_utf16(&self, range: Range<usize>) -> Range<usize> {
        self.text[..self.boundary(range.start)]
            .encode_utf16()
            .count()..self.text[..self.boundary(range.end)].encode_utf16().count()
    }

    pub(crate) fn range_from_utf16(&self, range: Range<usize>) -> Range<usize> {
        self.byte_for_utf16(range.start)..self.byte_for_utf16(range.end)
    }

    pub(crate) fn select_utf16(&mut self, range: Range<usize>) {
        let range = self.range_from_utf16(range);
        self.anchor = range.start;
        self.cursor = range.end;
    }

    fn boundary(&self, offset: usize) -> usize {
        let mut offset = offset.min(self.text.len());
        while !self.text.is_char_boundary(offset) {
            offset -= 1;
        }
        offset
    }

    fn byte_for_utf16(&self, target: usize) -> usize {
        let mut units = 0;
        for (offset, ch) in self.text.char_indices() {
            if units + ch.len_utf16() > target {
                return offset;
            }
            units += ch.len_utf16();
        }
        self.text.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_replaces_and_arrows_preserve_unicode_boundaries() {
        let mut edit = TextEdit::new("字体🙂".into());
        edit.move_to(edit.text.len(), false);
        edit.delete(true);
        assert_eq!(edit.text, "字体");
        edit.move_character(false, false);
        edit.replace(None, "号", false);
        assert_eq!(edit.text, "字号体");
        edit.select_all();
        edit.replace(None, "ffffff", false);
        assert_eq!(edit.text, "ffffff");
    }

    #[test]
    fn utf16_and_preedit_replace_without_duplicate_text() {
        let mut edit = TextEdit::new("a🙂b".into());
        assert_eq!(edit.range_from_utf16(1..3), 1..5);
        edit.select_utf16(1..3);
        edit.replace(None, "字", true);
        assert_eq!(edit.to_utf16(edit.marked.clone().unwrap()), 1..2);
        edit.replace(None, "字体", false);
        assert_eq!(edit.text, "a字体b");
        assert!(edit.marked.is_none());
    }

    #[test]
    fn input_is_single_line_and_bounded() {
        let mut edit = TextEdit::default();
        edit.replace(None, &"中\n".repeat(400), false);
        assert_eq!(edit.text.chars().count(), 256);
        edit.replace(None, "x", false);
        assert_eq!(edit.text.chars().count(), 256);
    }
}
