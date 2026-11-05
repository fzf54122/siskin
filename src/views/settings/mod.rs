// @Time    : 2026/11/4 23:30
// @Author  : fzf
// @FileName: mod.rs
// @Software: Siskin

mod appearance;
mod colors;
mod desktop;
mod fonts;
mod shell;

use super::*;

impl SiskinApp {
    pub(super) fn settings_page(&self, p: &Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        div()
            .id("settings-page")
            .debug_selector(|| "settings-page".into())
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(p.canvas))
            .child(self.settings_navigation(p, cx))
            .child(
                div()
                    .id("settings-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(
                        div()
                            .w_full()
                            .max_w(px(960.))
                            .mx_auto()
                            .p(px(28.))
                            .child(self.settings_heading(p))
                            .child(self.settings_content(p, cx)),
                    ),
            )
    }

    fn settings_navigation(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .px(px(20.))
            .h(px(52.))
            .flex_shrink_0()
            .bg(rgb(p.window))
            .border_b_1()
            .border_color(rgb(p.border))
            .children(
                SettingsSection::ALL
                    .into_iter()
                    .enumerate()
                    .map(|(index, section)| self.settings_nav_item(index, section, p, cx)),
            )
            .child(div().flex_1())
            .child(action("settings-done", "返回", p, cx, |app| {
                app.set_page(Page::Terminal)
            }))
    }

    fn settings_nav_item(
        &self,
        index: usize,
        section: SettingsSection,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let active = self.settings_section == section;
        div()
            .id(("settings-section", index))
            .debug_selector(move || format!("settings-section-{index}"))
            .px(px(12.))
            .py(px(8.))
            .rounded(px(6.))
            .border_1()
            .border_color(rgb(if active { p.accent_soft } else { p.window }))
            .bg(rgb(if active { p.tint } else { p.window }))
            .text_color(rgb(if active { p.accent } else { p.muted }))
            .text_size(px(13.))
            .cursor_pointer()
            .on_click(cx.listener(move |app, _, _, cx| {
                app.select_settings_section(section);
                cx.notify();
            }))
            .child(section.title())
    }

    fn settings_heading(&self, p: &Palette) -> Div {
        div()
            .mb(px(28.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        label(self.settings_section.title(), p.ink)
                            .text_size(px(25.))
                            .font_weight(FontWeight::SEMIBOLD),
                    )
                    .child(detail("自动保存", p)),
            )
            .child(detail(self.settings_section.description(), p).mt(px(8.)))
    }

    fn settings_content(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        match self.settings_section {
            SettingsSection::Appearance => self.appearance_settings(p, cx),
            SettingsSection::Typography => self.font_settings(p, cx),
            SettingsSection::Shell => self.shell_settings(p, cx),
            SettingsSection::Desktop => self.desktop_settings(p, cx),
        }
    }
}
