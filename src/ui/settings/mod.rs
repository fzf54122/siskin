// @Time    : 2026/11/4 23:30
// @Author  : fzf
// @FileName: mod.rs
// @Software: Siskin

mod appearance;
mod colors;
mod desktop;
mod fonts;
mod shell;

use super::sidebar::{side_action, side_rail};
use super::*;

impl SiskinApp {
    pub(super) fn settings_page(&self, p: &Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        div()
            .id("settings-page")
            .debug_selector(|| "settings-page".into())
            .size_full()
            .bg(rgb(p.window))
            .flex()
            .flex_col()
            .child(
                div()
                    .id("settings-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.settings_scroll)
                    .child(
                        div()
                            .w_full()
                            .max_w(px(856.))
                            .px(px(48.))
                            .pt(px(40.))
                            .pb(px(28.))
                            .child(self.settings_heading(p, cx))
                            .child(self.settings_content(p, cx))
                            .child(self.settings_footer(p, cx)),
                    ),
            )
    }

    pub(super) fn settings_sidebar(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        side_rail(p, self.settings.glass_surface(p.sidebar, p.dark))
            .child(
                side_action(
                    "settings-done",
                    "返回终端",
                    "",
                    ChromeIcon::Back,
                    p,
                    cx,
                    |app| {
                        app.dispatch(crate::domain::Action::OpenTerminal);
                    },
                )
                .h(px(HEADING_HEIGHT))
                .mx(px(12.))
                .w_auto(),
            )
            .child(
                div().px(px(12.)).pt(px(24.)).flex_1().min_h_0().children(
                    SettingsSection::ALL
                        .into_iter()
                        .enumerate()
                        .map(|(index, section)| self.settings_nav_item(index, section, p, cx)),
                ),
            )
            .child(
                detail(format!("Siskin  {}", env!("CARGO_PKG_VERSION")), p)
                    .text_size(px(10.))
                    .px(px(22.))
                    .py(px(16.)),
            )
    }

    fn settings_nav_item(
        &self,
        index: usize,
        section: SettingsSection,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let active = self.settings_section == section;
        let icon = match section {
            SettingsSection::Appearance => ChromeIcon::Palette,
            SettingsSection::Typography => ChromeIcon::Type,
            SettingsSection::Shell => ChromeIcon::Terminal,
            SettingsSection::Desktop => ChromeIcon::Desktop,
        };
        div()
            .id(("settings-section", index))
            .debug_selector(move || format!("settings-section-{index}"))
            .relative()
            .h(px(40.))
            .mb(px(5.))
            .px(px(10.))
            .flex()
            .items_center()
            .gap(px(10.))
            .rounded(px(3.))
            .border_1()
            .border_color(rgb(if active { p.border } else { p.sidebar }))
            .focusable()
            .cursor_pointer()
            .hover(|style| style.border_color(rgb(p.border)))
            .focus(|style| style.border_color(rgb(p.accent)))
            .on_click(cx.listener(move |app, _, _, cx| {
                app.select_settings_section(section);
                cx.notify();
            }))
            .on_key_down(cx.listener(move |app, event: &KeyDownEvent, _, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    app.select_settings_section(section);
                    cx.notify();
                    cx.stop_propagation();
                }
            }))
            .when(active, |view| {
                view.child(
                    div()
                        .absolute()
                        .left(px(-1.))
                        .top(px(-1.))
                        .bottom(px(-1.))
                        .w(px(3.))
                        .bg(rgb(p.ink)),
                )
            })
            .child(chrome_icon(icon, if active { p.ink } else { p.muted }))
            .child(
                label(section.title(), p.ink)
                    .text_size(px(13.))
                    .when(active, |view| view.font_weight(FontWeight::MEDIUM)),
            )
    }

    fn settings_heading(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        div()
            .mb(px(32.))
            .child(
                div()
                    .flex()
                    .items_start()
                    .justify_between()
                    .child(
                        div()
                            .child(
                                label(self.settings_section.title(), p.ink)
                                    .text_size(px(22.))
                                    .font_weight(FontWeight::MEDIUM),
                            )
                            .child(detail(self.settings_section.description(), p).mt(px(7.))),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(5.))
                            .pt(px(8.))
                            .when(self.save_error.is_none(), |view| {
                                view.child(chrome_icon(ChromeIcon::Check, p.muted).size(px(12.)))
                            })
                            .child(detail(self.save_label(), p).text_size(px(10.))),
                    ),
            )
            .when_some(self.save_error.clone(), |view, error| {
                view.child(
                    div()
                        .id("settings-save-error")
                        .debug_selector(|| "settings-save-error".into())
                        .mt(px(16.))
                        .p(px(12.))
                        .border_1()
                        .border_color(rgb(p.error))
                        .rounded(px(3.))
                        .child(label("未能保存设置", p.error).text_size(px(13.)))
                        .child(detail(format!("{error}；改动仍在本窗口生效。"), p).mt(px(6.)))
                        .child(
                            action("settings-save-retry", "重试", p, cx, |app| {
                                app.save_preferences()
                            })
                            .mt(px(10.)),
                        ),
                )
            })
    }

    fn settings_content(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        match self.settings_section {
            SettingsSection::Appearance => self.appearance_settings(p, cx),
            SettingsSection::Typography => self.font_settings(p, cx),
            SettingsSection::Shell => self.shell_settings(p, cx),
            SettingsSection::Desktop => self.desktop_settings(p, cx),
        }
    }

    fn settings_footer(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        div()
            .mt(px(48.))
            .pt(px(17.))
            .border_t_1()
            .border_color(rgb(p.hairline))
            .flex()
            .items_center()
            .gap(px(8.))
            .child(
                detail(self.preferences_label(), p)
                    .text_size(px(10.))
                    .flex_1()
                    .min_w_0()
                    .truncate(),
            )
            .child(
                chrome_action(
                    "reveal-preferences",
                    "在文件夹中显示 ↗",
                    p,
                    cx,
                    |app| app.reveal_preferences(),
                )
                .text_size(px(10.))
                .bg(rgb(p.window)),
            )
    }
}
