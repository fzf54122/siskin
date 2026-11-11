// @Time    : 2026/11/4 23:30
// @Author  : fzf
// @FileName: colors.rs
// @Software: Siskin

use super::*;

impl SiskinApp {
    pub(super) fn custom_colors(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        group()
            .child(group_heading("终端配色", "", p))
            .child(form_row(
                "配色方案",
                "应用于终端文字与 ANSI 颜色。",
                div().h(px(33.)).flex().items_center().child(detail(
                    if p.id == theme::ThemeId::Custom {
                        "自定义"
                    } else {
                        // 预设名就是当前主题，避免只剩“深色/浅色”分不清是哪一套。
                        p.id.name()
                    },
                    p,
                )),
                p,
            ))
            .when(self.settings.mode == ThemeMode::Custom, |view| {
                view.child(form_row(
                    "基础色调",
                    "",
                    action(
                        "custom-dark",
                        if self.settings.custom.dark {
                            "深色 · 切换"
                        } else {
                            "浅色 · 切换"
                        },
                        p,
                        cx,
                        |app| app.toggle_custom_dark(),
                    )
                    .w_full(),
                    p,
                ))
            })
            .child(self.color_field(
                "color-background",
                "背景颜色",
                ColorTarget::Background,
                p,
                cx,
            ))
            .child(self.color_field(
                "color-foreground",
                "文字颜色",
                ColorTarget::Foreground,
                p,
                cx,
            ))
            .child(
                chrome_action("reset-colors", "恢复默认配色", p, cx, |app| {
                    app.reset_colors()
                })
                .px(px(0.))
                .mt(px(12.))
                .text_size(px(11.))
                .bg(rgb(p.window)),
            )
    }

    pub(super) fn color_field(
        &self,
        id: &'static str,
        title: &'static str,
        target: ColorTarget,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Div {
        let error = self
            .color_edit
            .as_ref()
            .filter(|edit| edit.target == target)
            .and_then(|edit| edit.error.clone());
        div()
            .child(form_row(
                title,
                "",
                self.color_control(id, target, p, cx),
                p,
            ))
            .when_some(error, |view, error| {
                view.child(label(error, p.error).text_size(px(11.)).mt(px(6.)))
            })
    }

    fn color_control(
        &self,
        id: &'static str,
        target: ColorTarget,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let color = match target {
            ColorTarget::Accent => p.accent,
            ColorTarget::Background => p.terminal,
            ColorTarget::Foreground => p.ink,
        };
        let edit = self
            .color_edit
            .as_ref()
            .filter(|edit| edit.target == target);
        div()
            .id(id)
            .debug_selector(move || id.into())
            .h(px(33.))
            .w_full()
            .px(px(10.))
            .flex()
            .items_center()
            .gap(px(8.))
            .rounded(px(3.))
            .border_1()
            .border_color(rgb(if edit.is_some() { p.accent } else { p.border }))
            .bg(rgb(p.window))
            .cursor_text()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |app, _, window, cx| {
                    if app
                        .color_edit
                        .as_ref()
                        .is_none_or(|edit| edit.target != target)
                    {
                        app.edit_color(target);
                    }
                    app.focus.focus(window, cx);
                    cx.notify();
                }),
            )
            .child(
                div()
                    .size(px(14.))
                    .flex_shrink_0()
                    .rounded(px(2.))
                    .bg(rgb(color))
                    .border_1()
                    .border_color(rgb(p.border)),
            )
            .when(edit.is_none(), |view| {
                view.child(
                    label(format!("#{color:06X}"), p.muted)
                        .text_size(px(11.))
                        .font_family(self.font()),
                )
            })
            .when(edit.is_some(), |view| {
                view.child(detail("#", p))
                    .child(self.edit_field(
                        match target {
                            ColorTarget::Accent => "edit-accent",
                            ColorTarget::Background => "edit-background",
                            ColorTarget::Foreground => "edit-foreground",
                        },
                        p,
                        cx,
                    ))
                    .child(self.color_edit_button(id, true, p, cx))
                    .child(self.color_edit_button(id, false, p, cx))
            })
    }

    fn color_edit_button(
        &self,
        field: &'static str,
        apply: bool,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let id = format!("{field}-{}", if apply { "apply" } else { "cancel" });
        div()
            .id(id.clone())
            .debug_selector(move || id.clone())
            .size(px(18.))
            .flex_shrink_0()
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(move |app, _, _, cx| {
                if apply {
                    app.commit_color();
                } else {
                    app.color_edit = None;
                }
                cx.stop_propagation();
                cx.notify();
            }))
            .child(chrome_icon(
                if apply {
                    ChromeIcon::Check
                } else {
                    ChromeIcon::Close
                },
                p.muted,
            ))
    }
}
