// @Time    : 2026/11/5 17:42
// @Author  : fzf
// @FileName: input_handler.rs
// @Software: Siskin

use super::*;
use gpui::{Bounds, EntityInputHandler, Pixels, UTF16Selection};
use std::ops::Range;

impl EntityInputHandler for SiskinApp {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        actual: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let edit = self.editor()?;
        let range = edit.range_from_utf16(range);
        *actual = Some(edit.to_utf16(range.clone()));
        Some(edit.text[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(self.editor().map_or(
            UTF16Selection {
                range: 0..0,
                reversed: false,
            },
            |edit| UTF16Selection {
                range: edit.to_utf16(edit.selection()),
                reversed: edit.cursor < edit.anchor,
            },
        ))
    }

    fn set_selected_text_range(
        &mut self,
        range: Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(edit) = self.editor_mut() {
            edit.select_utf16(range);
            window.notify_text_selection_changed(&self.focus);
            cx.notify();
        }
    }

    fn text_length_utf16(&mut self, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        self.editor().map(|edit| edit.text.encode_utf16().count())
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        if let Some(edit) = self.editor() {
            edit.marked.clone().map(|range| edit.to_utf16(range))
        } else if self.page == Page::Terminal {
            self.session()
                .marked
                .as_ref()
                .map(|text| 0..text.encode_utf16().count())
        } else {
            None
        }
    }

    fn unmark_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(edit) = self.editor_mut() {
            edit.marked = None;
        } else {
            self.session_mut().marked = None;
        }
        window.notify_text_input_changed(&self.focus);
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(edit) = self.editor_mut() {
            edit.replace(range, text, false);
            self.editor_changed();
        } else if self.palette.is_some() {
            for ch in text.chars() {
                self.palette_type(ch);
            }
        } else if self.search.is_some() {
            for ch in text.chars() {
                self.search_type(ch);
            }
        } else if self.page == Page::Terminal && self.close_request.is_none() {
            self.session_mut().marked = None;
            self.paste_text(text);
        }
        window.notify_text_input_changed(&self.focus);
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(edit) = self.editor_mut() {
            edit.replace(range, text, true);
            if let Some(marked) = edit.marked.clone()
                && let Some(selected) = selected
            {
                let start = edit.to_utf16(marked).start;
                edit.select_utf16(start + selected.start..start + selected.end);
            }
            self.editor_changed();
        } else if self.page == Page::Terminal && self.close_request.is_none() {
            self.session_mut().marked = (!text.is_empty()).then(|| text.to_string());
        }
        window.notify_text_input_changed(&self.focus);
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        self.cursor_slot.lock().ok().and_then(|slot| *slot)
    }

    fn character_index_for_point(
        &mut self,
        _: gpui::Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        self.editor()
            .map(|edit| edit.to_utf16(edit.cursor..edit.cursor).start)
    }
}
