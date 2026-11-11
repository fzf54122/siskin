// @Time    : 2026/11/5 17:42
// @Author  : fzf
// @FileName: editor_ops.rs
// @Software: Siskin

use gpui::{Context, Keystroke};

use super::{SiskinApp, keys::KeyPress};
use crate::domain::text_edit::TextEdit;

#[derive(Default)]
pub(crate) struct FontPicker {
    pub query: TextEdit,
    pub selected: usize,
    pub scroll: gpui::ScrollHandle,
}

impl SiskinApp {
    pub(crate) fn editor(&self) -> Option<&TextEdit> {
        self.color_edit
            .as_ref()
            .map(|edit| &edit.draft)
            .or_else(|| self.font_picker.as_ref().map(|picker| &picker.query))
            // 标签重命名共用同一套输入法与剪贴板入口：
            // `keys.rs` 的 rename_stage 排在 color_edit_stage 之前，
            // 改名打开时按键不会落到这里的取色/字体分支。
            .or_else(|| self.tab_rename.as_ref().map(|rename| &rename.edit))
    }

    pub(crate) fn editor_mut(&mut self) -> Option<&mut TextEdit> {
        if let Some(edit) = self.color_edit.as_mut() {
            Some(&mut edit.draft)
        } else if let Some(picker) = self.font_picker.as_mut() {
            Some(&mut picker.query)
        } else {
            self.tab_rename.as_mut().map(|rename| &mut rename.edit)
        }
    }

    pub(crate) fn font_matches(&self) -> Vec<(usize, String)> {
        let query = self
            .font_picker
            .as_ref()
            .map(|picker| picker.query.text.to_lowercase())
            .unwrap_or_default();
        self.mono_fonts
            .iter()
            .enumerate()
            .filter(|(_, name)| name.to_lowercase().contains(&query))
            .map(|(index, name)| (index, name.clone()))
            .collect()
    }

    pub(crate) fn toggle_font_picker(&mut self) {
        self.color_edit = None;
        self.font_picker = self.font_picker.is_none().then(FontPicker::default);
        self.request_focus = true;
    }

    pub(crate) fn choose_font(&mut self, index: usize) {
        if index < self.mono_fonts.len() {
            self.settings.font_family = index;
            self.font_picker = None;
            self.commit_settings();
            self.request_focus = true;
        }
    }

    pub(crate) fn editor_changed(&mut self) {
        if let Some(edit) = self.color_edit.as_mut() {
            edit.error = None;
        }
        if let Some(picker) = self.font_picker.as_mut() {
            picker.selected = 0;
        }
    }

    pub(crate) fn edit_key(&mut self, press: &KeyPress<'_>) {
        match press.key {
            "escape" | "tab" => {
                self.color_edit = None;
                self.font_picker = None;
            }
            "enter" if self.color_edit.is_some() => self.commit_color(),
            "enter" => {
                let selected = self
                    .font_picker
                    .as_ref()
                    .map_or(0, |picker| picker.selected);
                if let Some((index, _)) = self.font_matches().get(selected) {
                    self.choose_font(*index);
                }
            }
            "up" | "down" if self.font_picker.is_some() => {
                let count = self.font_matches().len();
                if let Some(picker) = self.font_picker.as_mut() {
                    picker.selected = if press.key == "up" {
                        picker.selected.saturating_sub(1)
                    } else {
                        (picker.selected + 1).min(count.saturating_sub(1))
                    };
                    picker.scroll.scroll_to_item(picker.selected);
                }
            }
            _ => self.edit_text_key(press),
        }
    }

    fn edit_text_key(&mut self, press: &KeyPress<'_>) {
        let Some(edit) = self.editor_mut() else {
            return;
        };
        match press.key {
            "a" if press.control || press.platform => edit.select_all(),
            "left" => edit.move_character(false, press.shift),
            "right" => edit.move_character(true, press.shift),
            "home" => edit.move_to(0, press.shift),
            "end" => edit.move_to(edit.text.len(), press.shift),
            "backspace" => edit.delete(true),
            "delete" => edit.delete(false),
            _ if !press.modified() => {
                if let Some(text) = press
                    .key_char
                    .or_else(|| (press.key.len() == 1).then_some(press.key))
                {
                    edit.replace(None, text, false);
                }
            }
            _ => {}
        }
        self.editor_changed();
    }

    pub(crate) fn editor_clipboard(&mut self, key: &Keystroke, cx: &mut Context<Self>) -> bool {
        if !(key.modifiers.control || key.modifiers.platform) || self.editor().is_none() {
            return false;
        }
        match key.key.as_str() {
            "c" | "x" => {
                if let Some(edit) = self.editor() {
                    let text = edit.text[edit.selection()].to_string();
                    if !text.is_empty() {
                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
                    }
                }
                if key.key == "x"
                    && let Some(edit) = self.editor_mut()
                {
                    edit.replace(None, "", false);
                    self.editor_changed();
                }
            }
            "v" => {
                if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text())
                    && let Some(edit) = self.editor_mut()
                {
                    edit.replace(None, &text, false);
                    self.editor_changed();
                }
            }
            _ => return false,
        }
        true
    }
}
