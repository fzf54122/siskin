// @Time    : 2026/11/4 23:30
// @Author  : fzf
// @FileName: colors.rs
// @Software: Siskin

use super::*;

impl SiskinApp {
    pub(super) fn custom_colors(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        group(p)
            .child(group_heading(
                "自定义配色",
                "从一个基础色调开始，再调整终端的颜色。",
                p,
            ))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .mb(px(20.))
                    .child(label("基础色调", p.ink))
                    .child(action(
                        "custom-dark",
                        if self.settings.custom.dark {
                            "深色  /  切换"
                        } else {
                            "浅色  /  切换"
                        },
                        p,
                        cx,
                        |app| app.toggle_custom_dark(),
                    )),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .mb(px(20.))
                    .child(label("强调色", p.ink))
                    .child(
                        div().flex().gap(px(8.)).children(
                            theme::ACCENTS
                                .iter()
                                .enumerate()
                                .map(|(index, (color, _))| {
                                    self.accent_swatch(index, *color, p, cx)
                                }),
                        ),
                    ),
            )
            .child(self.color_field("color-accent", "强调色值", ColorTarget::Accent, p, cx))
            .child(self.color_field(
                "color-background",
                "终端背景",
                ColorTarget::Background,
                p,
                cx,
            ))
            .child(self.color_field(
                "color-foreground",
                "终端文字",
                ColorTarget::Foreground,
                p,
                cx,
            ))
            .child(detail("点击色值编辑 · Ctrl A 重输 · Enter 应用 · Esc 取消", p).mt(px(14.)))
            .when(
                self.color_edit
                    .as_ref()
                    .and_then(|edit| edit.error.as_ref())
                    .is_some(),
                |view| {
                    view.child(
                        label(
                            self.color_edit
                                .as_ref()
                                .and_then(|edit| edit.error.clone())
                                .unwrap_or_default(),
                            p.error,
                        )
                        .text_size(px(12.))
                        .mt(px(8.)),
                    )
                },
            )
    }

    fn accent_swatch(
        &self,
        index: usize,
        color: u32,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let selected =
            self.settings.custom.accent_rgb.is_none() && self.settings.custom.accent == index;
        div()
            .id(("accent", index))
            .debug_selector(move || format!("accent-{index}"))
            .size(px(28.))
            .rounded_full()
            .border_2()
            .border_color(rgb(if selected { p.ink } else { p.window }))
            .bg(rgb(color))
            .cursor_pointer()
            .on_click(cx.listener(move |app, _, _, cx| {
                app.set_custom_accent(index);
                cx.notify();
            }))
    }

    fn color_field(
        &self,
        id: &'static str,
        title: &'static str,
        target: ColorTarget,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let custom = Palette::custom(self.settings.custom);
        let color = match target {
            ColorTarget::Accent => custom.accent,
            ColorTarget::Background => custom.terminal,
            ColorTarget::Foreground => custom.output,
        };
        let edit = self
            .color_edit
            .as_ref()
            .filter(|edit| edit.target == target);
        let value = edit.map_or_else(
            || format!("#{color:06x}"),
            |edit| format!("#{}▏", edit.draft),
        );
        div()
            .id(id)
            .debug_selector(move || id.into())
            .flex()
            .items_center()
            .justify_between()
            .h(px(48.))
            .border_t_1()
            .border_color(rgb(p.hairline))
            .cursor_text()
            .on_click(cx.listener(move |app, _, window, cx| {
                app.edit_color(target);
                app.focus.focus(window, cx);
                cx.notify();
            }))
            .child(label(title, p.ink))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(
                        div()
                            .size(px(18.))
                            .rounded(px(4.))
                            .bg(rgb(color))
                            .border_1()
                            .border_color(rgb(p.border)),
                    )
                    .child(
                        label(value, if edit.is_some() { p.accent } else { p.muted })
                            .font_family(self.font())
                            .text_size(px(13.)),
                    ),
            )
    }
}
