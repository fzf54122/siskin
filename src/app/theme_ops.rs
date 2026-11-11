// @Time    : 2026/11/5 13:40
// @Author  : fzf
// @FileName: theme_ops.rs
// @Software: Siskin

//! 主题与取色。主题改动都要落盘，所以每个入口末尾都走
//! `commit_settings` / `save_preferences`，不在这里重复写文件。

use crate::settings::ThemeMode;
use crate::theme;

use super::{ColorEdit, ColorTarget, SiskinApp};

impl SiskinApp {
    pub(crate) fn set_theme_mode(&mut self, mode: ThemeMode) {
        self.settings.set_mode(mode, self.system_dark);
        self.color_edit = None;
        self.font_picker = None;
        self.commit_settings();
    }

    pub(crate) fn set_system_dark(&mut self, dark: bool) {
        self.system_dark = dark;
        self.settings.resolve_theme(dark);
    }

    pub(crate) fn toggle_custom_dark(&mut self) {
        self.settings.custom.dark = !self.settings.custom.dark;
        self.resolve_custom_theme();
        self.commit_settings();
    }

    fn resolve_custom_theme(&mut self) {
        if self.settings.mode == ThemeMode::Custom {
            self.settings.resolve_theme(self.system_dark);
        }
    }

    pub(crate) fn cycle_theme(&mut self) {
        self.settings.next_theme(self.system_dark);
        self.commit_settings();
    }

    /// 从命令面板点选一套i置主题；固定选择跟随设置一起落盘。
    pub(crate) fn set_theme(&mut self, theme: theme::ThemeId) {
        self.settings.set_theme(theme, self.system_dark);
        self.commit_settings();
    }

    pub(crate) fn edit_color(&mut self, target: ColorTarget) {
        let current = self.theme();
        let base = if self.settings.mode == ThemeMode::Custom {
            self.settings.custom
        } else {
            theme::CustomTheme {
                dark: current.dark,
                accent_rgb: Some(theme::ThemeColor(current.accent)),
                background: Some(theme::ThemeColor(current.terminal)),
                foreground: Some(theme::ThemeColor(current.ink)),
                ..Default::default()
            }
        };
        self.font_picker = None;
        let color = match target {
            ColorTarget::Accent => current.accent,
            ColorTarget::Background => current.terminal,
            ColorTarget::Foreground => current.ink,
        };
        self.color_edit = Some(ColorEdit {
            target,
            draft: crate::domain::text_edit::TextEdit::new(format!("{color:06x}")),
            error: None,
            base,
        });
        self.request_focus = true;
    }

    pub(crate) fn commit_color(&mut self) {
        let Some(edit) = self.color_edit.as_mut() else {
            return;
        };
        let color = match theme::ThemeColor::try_from(edit.draft.text.clone()) {
            Ok(color) => color,
            Err(error) => {
                edit.error = Some(error);
                return;
            }
        };
        self.settings.custom = edit.base;
        match edit.target {
            ColorTarget::Accent => self.settings.custom.accent_rgb = Some(color),
            ColorTarget::Background => self.settings.custom.background = Some(color),
            ColorTarget::Foreground => self.settings.custom.foreground = Some(color),
        }
        self.color_edit = None;
        self.set_theme_mode(ThemeMode::Custom);
    }

    pub(crate) fn font(&self) -> gpui::SharedString {
        self.mono_fonts
            .get(self.settings.font_family)
            .cloned()
            .unwrap_or_else(|| self.mono.to_string())
            .into()
    }

    pub(crate) fn font_size(&self) -> f32 {
        self.settings.font_size()
    }

    pub(crate) fn row_height(&self) -> f32 {
        self.settings.row_height()
    }

    pub(crate) fn step_font(&mut self, up: bool) {
        self.settings.step_font(up);
        self.commit_settings();
    }

    pub(crate) fn reset_colors(&mut self) {
        let mode = if self.theme().dark {
            ThemeMode::Dark
        } else {
            ThemeMode::Light
        };
        self.settings.custom = theme::CustomTheme::default();
        self.set_theme_mode(mode);
    }
}
