// @Time    : 2026/11/4 23:30
// @Author  : fzf
// @FileName: mod.rs
// @Software: Siskin

mod inspector;
mod palette;
mod preview;
mod settings;
mod sidebar;
mod terminal;
mod widgets;
mod workspace;

use crate::{
    app::{ColorTarget, SiskinApp},
    line::Line,
    settings::{FONT_SIZES, Page, SettingsSection, ShellKind, ThemeMode},
    theme::{self, Palette},
};
use gpui::{
    Context, Div, FontWeight, KeyDownEvent, MouseButton, SharedString, Stateful, Window, div,
    prelude::*, px, rgb, rgba,
};
use widgets::*;

impl Render for SiskinApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = self.theme();
        let width = window.viewport_size().width.as_f32();
        let show_inspector = self.inspector_open && self.page == Page::Terminal && width >= 1100.;
        let columns = ((width - 264. - if show_inspector { 280. } else { 0. }) / self.char_width())
            .floor()
            .max(40.) as usize;
        self.session_mut().terminal.set_columns(columns);
        div()
            .id("siskin-root")
            .relative()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                let key = &event.keystroke;
                if this.handle_key(
                    &key.key,
                    key.key_char.as_deref(),
                    key.modifiers.control,
                    key.modifiers.platform,
                    key.modifiers.alt,
                    key.modifiers.shift,
                ) {
                    cx.notify();
                    cx.stop_propagation();
                }
            }))
            .size_full()
            .font_family(self.ui.clone())
            .text_size(px(14.))
            .text_color(rgb(palette.ink))
            .bg(rgb(palette.canvas))
            .child(
                div()
                    .id("main-workspace")
                    .debug_selector(|| "main-workspace".into())
                    .size_full()
                    .flex()
                    .child(self.session_sidebar(&palette, cx))
                    .child(self.workspace(&palette, cx))
                    .when(show_inspector, |root| {
                        root.child(self.session_inspector(&palette, cx))
                    }),
            )
            .when(self.palette.is_some(), |root| {
                root.child(self.command_palette(&palette, cx))
            })
    }
}
