// @Time    : 2026/11/4 23:30
// @Author  : fzf
// @FileName: appearance.rs
// @Software: Siskin

use super::*;

impl SiskinApp {
    pub(super) fn appearance_settings(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .flex_col()
            .gap(px(24.))
            .child(
                div()
                    .child(group_heading(
                        "颜色模式",
                        "跟随环境，或为自己选择一种工作氛围。",
                        p,
                    ))
                    .child(
                        div().flex().gap(px(12.)).children(
                            ThemeMode::ALL
                                .into_iter()
                                .enumerate()
                                .map(|(index, mode)| self.theme_choice(index, mode, p, cx)),
                        ),
                    ),
            )
            .child(self.theme_preview(p))
            .when(self.settings.mode == ThemeMode::Custom, |view| {
                view.child(self.custom_colors(p, cx))
            })
            .child(detail(
                "跟随系统会自动响应桌面深浅色变化。手动选择不受系统切换影响。",
                p,
            ))
    }

    fn theme_choice(
        &self,
        index: usize,
        mode: ThemeMode,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let selected = self.settings.mode == mode;
        let preview = self.settings.palette_for(mode, self.system_dark);
        div()
            .id(("theme-mode", index))
            .debug_selector(move || format!("theme-mode-{index}"))
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(12.))
            .p(px(12.))
            .rounded(px(9.))
            .border_2()
            .border_color(rgb(if selected { p.accent } else { p.border }))
            .bg(rgb(p.window))
            .cursor_pointer()
            .on_click(cx.listener(move |app, _, _, cx| {
                app.set_theme_mode(mode);
                cx.notify();
            }))
            .child(theme_thumbnail(preview))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        label(mode.name(), if selected { p.accent } else { p.ink })
                            .text_size(px(13.))
                            .font_weight(FontWeight::MEDIUM),
                    )
                    .child(div().size(px(7.)).rounded_full().bg(rgb(if selected {
                        p.accent
                    } else {
                        p.window
                    }))),
            )
    }
}

fn theme_thumbnail(p: Palette) -> Div {
    div()
        .h(px(68.))
        .w_full()
        .rounded(px(5.))
        .overflow_hidden()
        .bg(rgb(p.terminal))
        .border_1()
        .border_color(rgb(p.border))
        .child(
            div()
                .h(px(13.))
                .bg(rgb(p.sidebar))
                .px(px(6.))
                .flex()
                .items_center()
                .child(div().w(px(26.)).h(px(4.)).rounded(px(2.)).bg(rgb(p.accent))),
        )
        .child(
            div()
                .p(px(8.))
                .flex()
                .flex_col()
                .gap(px(5.))
                .child(div().w(px(34.)).h(px(4.)).rounded(px(2.)).bg(rgb(p.ink)))
                .child(
                    div()
                        .w(px(50.))
                        .max_w_full()
                        .h(px(4.))
                        .rounded(px(2.))
                        .bg(rgb(p.muted)),
                )
                .child(div().w(px(22.)).h(px(4.)).rounded(px(2.)).bg(rgb(p.accent))),
        )
}
