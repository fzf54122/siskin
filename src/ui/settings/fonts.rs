// @Time    : 2026/11/4 23:30
// @Author  : fzf
// @FileName: fonts.rs
// @Software: Siskin

use super::*;

impl SiskinApp {
    pub(super) fn font_settings(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        div()
            .child(group_heading("终端文字", "", p))
            .child(form_row(
                "字体",
                "选择已安装的等宽字体，可搜索。",
                self.font_selector(p, cx),
                p,
            ))
            .child(form_row(
                "字号",
                "也可用 Ctrl + / Ctrl − 调整。",
                div()
                    .id("font-size-control")
                    .debug_selector(|| "font-size-control".into())
                    .h(px(33.))
                    .w_full()
                    .flex()
                    .items_center()
                    .rounded(px(3.))
                    .border_1()
                    .border_color(rgb(p.border))
                    .child(
                        chrome_action("font-down", "−", p, cx, |app| app.step_font(false))
                            .w(px(34.))
                            .bg(rgb(p.window)),
                    )
                    .child(
                        label(format!("{} px", self.font_size() as u32), p.ink)
                            .text_size(px(12.))
                            .flex_1()
                            .text_center(),
                    )
                    .child(
                        chrome_action("font-up", "+", p, cx, |app| app.step_font(true))
                            .w(px(34.))
                            .bg(rgb(p.window)),
                    ),
                p,
            ))
            .child(form_row(
                "行高",
                "根据字体度量自动调整。",
                detail("自动", p).h(px(33.)).flex().items_center(),
                p,
            ))
            .child(self.theme_preview(p).mt(px(28.)))
    }

    fn font_selector(&self, p: &Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        div()
            .id("font-selector")
            .debug_selector(|| "font-selector".into())
            .relative()
            .w_full()
            .child(
                action("toggle-font-picker", "", p, cx, |app| {
                    app.toggle_font_picker()
                })
                .h(px(33.))
                .w_full()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(8.))
                .child(
                    label(self.font(), p.ink)
                        .text_size(px(12.))
                        .min_w_0()
                        .flex_1()
                        .truncate(),
                )
                .child(label("⌄", p.muted).text_size(px(12.))),
            )
            .when(self.font_picker.is_some(), |view| {
                view.child(gpui::deferred(self.font_popup(p, cx)).with_priority(1))
            })
    }

    fn font_popup(&self, p: &Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let matches = self.font_matches();
        let selected = self
            .font_picker
            .as_ref()
            .map_or(0, |picker| picker.selected);
        div()
            .id("font-popup")
            .debug_selector(|| "font-popup".into())
            .absolute()
            .top(px(38.))
            .left(px(0.))
            .w(px(240.))
            .border_1()
            .border_color(rgb(p.border))
            .rounded(px(3.))
            .bg(rgb(p.window))
            .occlude()
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .relative()
                    .h(px(34.))
                    .px(px(9.))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .border_b_1()
                    .border_color(rgb(p.hairline))
                    .child(chrome_icon(ChromeIcon::Search, p.muted))
                    .child(self.edit_field("font-query", p, cx)),
            )
            .child(
                div()
                    .id("font-options")
                    .max_h(px(208.))
                    .overflow_y_scroll()
                    .when_some(self.font_picker.as_ref(), |view, picker| {
                        view.track_scroll(&picker.scroll)
                    })
                    .children(matches.iter().enumerate().map(|(position, (index, name))| {
                        self.font_choice(*index, name.clone(), position == selected, p, cx)
                    }))
                    .when(matches.is_empty(), |view| {
                        view.child(detail("没有匹配的等宽字体", p).p(px(12.)))
                    }),
            )
    }

    fn font_choice(
        &self,
        index: usize,
        name: String,
        selected: bool,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        div()
            .id(("font", index))
            .debug_selector(move || format!("font-{index}"))
            .h(px(32.))
            .px(px(10.))
            .flex()
            .items_center()
            .text_size(px(12.))
            .bg(rgb(if selected { p.tint } else { p.window }))
            .text_color(rgb(p.ink))
            .cursor_pointer()
            .hover(|style| style.bg(rgb(p.tint)))
            .on_click(cx.listener(move |app, _, _, cx| {
                app.choose_font(index);
                cx.stop_propagation();
                cx.notify();
            }))
            .child(name)
    }
}
