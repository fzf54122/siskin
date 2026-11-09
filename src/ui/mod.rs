// @Time    : 2026/11/5 17:30
// @Author  : fzf
// @FileName: mod.rs
// @Software: Siskin

//! GPUI 视图，不直接碰 `std::process`。会话、PTY、配置都在 `app` 与 `io`。

mod close;
mod grid;
mod icons;
mod input;
mod input_handler;
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
    settings::{Page, SettingsSection, ShellKind, ThemeMode},
    theme::{self, Palette},
};
use gpui::{
    Context, Div, FontWeight, KeyDownEvent, MouseButton, SharedString, Stateful, Window, div,
    prelude::*, px, rgb, rgba,
};
use icons::{ChromeIcon, chrome_icon};
use widgets::*;

// 设置页保留左侧分类导航，宽度比旧版会话侧栏窄。
const SETTINGS_RAIL_WIDTH: f32 = 224.;
const TAB_BAR_HEIGHT: f32 = 28.;
const HEADING_HEIGHT: f32 = 42.;
const INSPECTOR_WIDTH: f32 = 280.;

impl Render for SiskinApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = self.theme();
        let width = window.viewport_size().width.as_f32();
        let show_inspector = self.inspector_open && self.page == Page::Terminal && width >= 1100.;
        self.refresh_directory(false);
        if self.page == Page::Terminal {
            self.refresh_metrics(window, show_inspector);
        }
        if self.request_focus {
            self.focus.focus(window, cx);
            self.request_focus = false;
        }
        if self.request_quit {
            cx.quit();
        }
        self.sync_window_title(window);
        self.sync_clipboard(cx);
        self.flush_preferences_if_due();
        // 第二个实例转发过来的请求：先落成新会话，再刷新界面。
        if crate::io::instance::take_new_window_request() {
            self.new_session();
        }
        div()
            .id("siskin-root")
            .relative()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if this.sync_clipboard_shortcut(&event.keystroke, cx) {
                    cx.notify();
                    cx.stop_propagation();
                    return;
                }
                if let Some(handled) = this.handle_key(&event.keystroke) {
                    cx.notify();
                    cx.stop_propagation();
                    if matches!(handled, crate::app::Handled::Quit) {
                        cx.quit();
                    }
                }
            }))
            .size_full()
            .font_family(self.ui.clone())
            .text_size(px(14.))
            .text_color(rgb(palette.ink))
            // 半透明画布：窗口 surface 已切到 PreMultiplied alpha，这里给出
            // 0.92 的不透明度，桌面内容会从背后透出来。终端区域另用
            // `palette.terminal` 实底遮挡，保证输出可读。
            .bg(rgba((palette.canvas << 8) | (0.92 * 255.) as u32))
            .child(
                div()
                    .id("main-workspace")
                    .debug_selector(|| "main-workspace".into())
                    .size_full()
                    .flex()
                    .when(self.page == Page::Settings, |root| {
                        root.child(self.settings_sidebar(&palette, cx))
                    })
                    .child(self.workspace(&palette, cx))
                    .when(show_inspector, |root| {
                        root.child(self.session_inspector(&palette, cx))
                    }),
            )
            .when(self.palette.is_some(), |root| {
                root.child(self.command_palette(&palette, cx))
            })
            .when(self.close_request.is_some(), |root| {
                root.child(self.close_confirmation(&palette, cx))
            })
    }
}

impl SiskinApp {
    /// 窗口标题跟随最新 OSC 标题；没有时退回会话名。
    /// 只在变化时设置，避免每帧触发平台调用。
    fn sync_window_title(&mut self, window: &mut Window) {
        let title = match self.session().window_title.as_deref() {
            Some(title) => format!("{title} — Siskin"),
            None => format!("{} — Siskin", self.session().display_title()),
        };
        if self.window_title.as_ref() == Some(&title) {
            return;
        }
        window.set_window_title(&title);
        self.window_title = Some(title);
    }

    /// 用真实字形宽度推导 cell 尺寸，再据此算出网格行列数并同步给 PTY。
    /// 顺序必须是「先度量 → 再算行列 → 最后 resize」，否则会出现回环。
    fn refresh_metrics(&mut self, window: &mut Window, show_inspector: bool) {
        let font_size = px(self.font_size());
        let font = gpui::font(self.font());
        let font_id = window.text_system().resolve_font(&font);
        let advance = window.text_system().advance(font_id, font_size, 'm');
        let cell_width = advance
            .as_ref()
            .map_or(self.font_size() * 0.6, |size| size.width.as_f32());
        let ascent = window.text_system().ascent(font_id, font_size).as_f32();
        let descent = window.text_system().descent(font_id, font_size).as_f32();
        let line_height = ((ascent + descent) * 1.25).max(self.font_size() * 1.15);
        const PADDING: f32 = 24.;
        let viewport = window.viewport_size();
        // 侧栏已移除，终端可用宽度就是窗口宽度，只扣除右侧会话信息面板。
        let width = viewport.width.as_f32() - if show_inspector { INSPECTOR_WIDTH } else { 0. };
        // tab 条是终端上方唯一的常驻条，纵向可用高度只扣它。
        let height = viewport.height.as_f32() - TAB_BAR_HEIGHT;
        self.metrics =
            crate::domain::RenderMetrics::new(cell_width, line_height, self.font_size(), PADDING);
        let columns = self.metrics.columns(width);
        let rows = self.metrics.rows(height);
        self.resize_grid(columns, rows);
    }
}
