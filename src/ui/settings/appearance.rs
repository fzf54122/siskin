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
            .gap(px(28.))
            .child(
                group(p)
                    .child(group_heading("界面", "", p))
                    .child(form_row(
                        "颜色模式",
                        "跟随桌面，或使用固定的浅色、深色外观。",
                        div()
                            .w_full()
                            .h(px(33.))
                            .flex()
                            .rounded(px(3.))
                            .border_1()
                            .border_color(rgb(p.border))
                            .children(
                                ThemeMode::ALL
                                    .into_iter()
                                    .enumerate()
                                    .map(|(index, mode)| self.theme_choice(index, mode, p, cx)),
                            ),
                        p,
                    ))
                    .child(self.color_field("color-accent", "强调色", ColorTarget::Accent, p, cx)),
            )
            .child(self.custom_colors(p, cx))
    }

    fn theme_choice(
        &self,
        index: usize,
        mode: ThemeMode,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let selected = self.settings.mode == mode;
        div()
            .id(("theme-mode", index))
            .debug_selector(move || format!("theme-mode-{index}"))
            .relative()
            .h_full()
            .flex_1()
            .min_w_0()
            .flex()
            .items_center()
            .justify_center()
            .focusable()
            .text_size(px(11.))
            .text_color(rgb(if selected { p.ink } else { p.muted }))
            .cursor_pointer()
            .focus(|style| style.bg(rgb(p.tint)))
            .on_click(cx.listener(move |app, _, _, cx| {
                app.set_theme_mode(mode);
                cx.notify();
            }))
            .on_key_down(cx.listener(move |app, event: &KeyDownEvent, _, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    app.set_theme_mode(mode);
                    cx.notify();
                    cx.stop_propagation();
                }
            }))
            .when(index > 0, |view| {
                view.border_l_1().border_color(rgb(p.hairline))
            })
            .when(selected, |view| {
                view.font_weight(FontWeight::MEDIUM).child(
                    div()
                        .absolute()
                        .bottom(px(0.))
                        .left(px(10.))
                        .right(px(10.))
                        .h(px(2.))
                        .bg(rgb(p.accent)),
                )
            })
            .child(mode.name())
    }
}
