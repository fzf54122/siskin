// @Time    : 2026/11/4 23:30
// @Author  : fzf
// @FileName: widgets.rs
// @Software: Siskin

use super::*;

pub(super) fn label(value: impl Into<SharedString>, color: u32) -> Div {
    div()
        .text_size(px(14.))
        .text_color(rgb(color))
        .child(value.into())
}

pub(super) fn detail(value: impl Into<SharedString>, palette: &Palette) -> Div {
    pulse_ui::caption(value)
        .text_size(px(12.))
        .text_color(rgb(palette.muted))
}

pub(super) fn group(palette: &Palette) -> Div {
    pulse_ui::card()
        .p(px(20.))
        .rounded(px(10.))
        .bg(rgb(palette.window))
        .border_color(rgb(palette.border))
}

pub(super) fn group_heading(title: &'static str, note: &'static str, p: &Palette) -> Div {
    div()
        .mb(px(16.))
        .child(
            pulse_ui::heading(title)
                .text_size(px(15.))
                .text_color(rgb(p.ink)),
        )
        .child(detail(note, p).mt(px(5.)))
}

pub(super) fn action(
    id: &'static str,
    text: &'static str,
    p: &Palette,
    cx: &mut Context<SiskinApp>,
    action: impl Fn(&mut SiskinApp) + 'static,
) -> Stateful<Div> {
    // The library button fixes its hover color to the light palette. Chrome
    // controls need a single hover style resolved from the active app theme.
    div()
        .id(id)
        .debug_selector(move || id.into())
        .px(px(12.))
        .py(px(8.))
        .rounded(px(6.))
        .border_1()
        .cursor_pointer()
        .text_size(px(13.))
        .text_color(rgb(p.ink))
        .bg(rgb(p.window))
        .border_color(rgb(p.border))
        .hover(|style| style.bg(rgb(p.sidebar)))
        .on_click(cx.listener(move |this, _, _, cx| {
            action(this);
            cx.notify();
        }))
        .child(text)
}

pub(super) fn key_hint(text: &'static str, p: &Palette) -> Div {
    div()
        .px(px(6.))
        .py(px(2.))
        .rounded(px(4.))
        .bg(rgb(p.sidebar))
        .border_1()
        .border_color(rgb(p.border))
        .text_size(px(11.))
        .text_color(rgb(p.muted))
        .child(text)
}

pub(super) fn render_line(line: Line, p: &Palette, font_size: f32, row_height: f32) -> Div {
    div()
        .flex()
        .items_center()
        .h(px(row_height))
        .flex_shrink_0()
        .text_size(px(font_size))
        .whitespace_nowrap()
        .children(line.spans.into_iter().map(|span| {
            div()
                .text_color(rgb(span.role.color(p)))
                .when(span.role.is_bold(), |view| {
                    view.font_weight(FontWeight::SEMIBOLD)
                })
                .child(SharedString::from(span.text))
        }))
}
