// @Time    : 2026/11/4 23:30
// @Author  : fzf
// @FileName: fonts.rs
// @Software: Siskin

use super::*;

impl SiskinApp {
    pub(super) fn font_settings(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .flex_col()
            .gap(px(24.))
            .child(self.theme_preview(p))
            .child(
                group(p)
                    .child(group_heading(
                        "终端字体",
                        "仅列出当前系统已安装的等宽字体。",
                        p,
                    ))
                    .children(
                        self.mono_fonts
                            .iter()
                            .enumerate()
                            .map(|(index, font)| self.font_choice(index, font, p, cx)),
                    ),
            )
            .child(self.font_size_control(p, cx))
    }

    fn font_choice(
        &self,
        index: usize,
        font: &str,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let selected = self.settings.font_family == index;
        div()
            .id(("font", index))
            .debug_selector(move || format!("font-{index}"))
            .flex()
            .items_center()
            .justify_between()
            .h(px(48.))
            .px(px(12.))
            .rounded(px(6.))
            .bg(rgb(if selected { p.tint } else { p.window }))
            .cursor_pointer()
            .hover(|style| style.bg(rgb(p.sidebar)))
            .on_click(cx.listener(move |app, _, _, cx| {
                app.settings.font_family = index;
                app.save_preferences();
                cx.notify();
            }))
            .child(
                label(font.to_string(), if selected { p.accent } else { p.ink })
                    .font_family(font.to_string())
                    .text_size(px(13.)),
            )
            .child(
                label("Aa 012345", p.muted)
                    .font_family(font.to_string())
                    .text_size(px(13.)),
            )
    }

    fn font_size_control(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        group(p)
            .child(group_heading(
                "字号与行距",
                "等宽字符保持一致的节奏，随字号同步调整行高。",
                p,
            ))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .child(label(format!("{} px", self.font_size() as u32), p.ink))
                            .child(
                                detail(format!("行高 {} px", self.row_height() as u32), p)
                                    .mt(px(4.)),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(12.))
                            .child(action("font-down", "A−", p, cx, |app| {
                                app.step_font(false)
                            }))
                            .child(detail(
                                format!("{} / {}", self.settings.font_index + 1, FONT_SIZES.len()),
                                p,
                            ))
                            .child(action("font-up", "A+", p, cx, |app| app.step_font(true))),
                    ),
            )
    }
}
