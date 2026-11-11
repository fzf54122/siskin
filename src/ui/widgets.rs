// @Time    : 2026/11/4 23:30
// @Author  : fzf
// @FileName: widgets.rs
// @Software: Siskin

use gpui::{HighlightStyle, Hsla, StyledText};

use super::*;

pub(super) fn label(value: impl Into<SharedString>, color: u32) -> Div {
    div()
        .text_size(px(14.))
        .text_color(rgb(color))
        .child(value.into())
}

/// 带模糊命中高亮的标签：命中字符用强调色加粗，其余用默认色。
/// `ranges` 为空时等价于 `label`。
pub(super) fn highlight_label(text: &str, ranges: &[(usize, usize)], p: &Palette) -> Div {
    if ranges.is_empty() {
        return label(text, p.ink);
    }
    let highlights = ranges.iter().map(|(start, end)| {
        (
            *start..*end,
            HighlightStyle {
                color: Some(Hsla::from(rgb(p.accent))),
                font_weight: Some(FontWeight::BOLD),
                ..Default::default()
            },
        )
    });
    div()
        .text_size(px(14.))
        .text_color(rgb(p.ink))
        .child(StyledText::new(text).with_highlights(highlights))
}

pub(super) fn detail(value: impl Into<SharedString>, palette: &Palette) -> Div {
    pulse_ui::caption(value)
        .text_size(px(12.))
        .text_color(rgb(palette.muted))
}

pub(super) fn group() -> Div {
    // 不再起卡片底色：设置页透出窗口底色，分组之间只靠间距区分，
    // 避免「卡片里嵌卡片」的嵌套观感。
    div()
}

pub(super) fn group_heading(title: &'static str, note: &'static str, p: &Palette) -> Div {
    div()
        .mb(px(2.))
        .pb(px(10.))
        .child(
            pulse_ui::heading(title)
                .text_size(px(12.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgb(p.ink)),
        )
        .when(!note.is_empty(), |view| {
            view.child(detail(note, p).text_size(px(11.)).mt(px(5.)))
        })
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
    let action = std::rc::Rc::new(action);
    let keyboard_action = action.clone();
    div()
        .id(id)
        .debug_selector(move || id.into())
        .px(px(10.))
        .py(px(6.))
        .rounded(px(3.))
        .border_1()
        .focusable()
        .cursor_pointer()
        .text_size(px(13.))
        .text_color(rgb(p.ink))
        .bg(rgb(p.window))
        .border_color(rgb(p.border))
        .hover(|style| style.bg(rgb(p.sidebar)))
        .focus(|style| style.border_color(rgb(p.accent)))
        .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
            if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                keyboard_action(this);
                cx.notify();
                cx.stop_propagation();
            }
        }))
        .on_click(cx.listener(move |this, _, _, cx| {
            action(this);
            cx.notify();
        }))
        .child(text)
}

pub(super) fn form_row(
    title: &'static str,
    note: &'static str,
    control: impl IntoElement,
    p: &Palette,
) -> Div {
    div()
        .w_full()
        .min_h(px(72.))
        .py(px(18.))
        .flex()
        .items_center()
        .justify_between()
        .gap(px(24.))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(label(title, p.ink).text_size(px(13.)))
                .when(!note.is_empty(), |view| {
                    view.child(detail(note, p).text_size(px(11.)).mt(px(5.)))
                }),
        )
        .child(div().w(px(240.)).flex_shrink_0().child(control))
}

pub(super) fn chrome_action(
    id: &'static str,
    text: &'static str,
    p: &Palette,
    cx: &mut Context<SiskinApp>,
    callback: impl Fn(&mut SiskinApp) + 'static,
) -> Stateful<Div> {
    action(id, text, p, cx, callback)
        .border_0()
        .h(px(28.))
        .flex()
        .items_center()
        .justify_center()
        .bg(rgb(p.sidebar))
}

pub(super) fn icon_action(
    id: &'static str,
    icon: ChromeIcon,
    tip: &'static str,
    selected: bool,
    action: crate::domain::Action,
    p: &Palette,
    cx: &mut Context<SiskinApp>,
) -> Stateful<Div> {
    let palette = *p;
    chrome_action(id, "", p, cx, move |app| {
        app.dispatch(action);
    })
    .w(px(24.))
    .h(px(24.))
    .px(px(0.))
    .when(selected, |view| view.bg(rgb(p.tint)))
    .tooltip(move |_, cx| {
        cx.new(|_| Hint {
            text: tip.into(),
            palette,
        })
        .into()
    })
    .child(chrome_icon(icon, if selected { p.ink } else { p.muted }))
}

struct Hint {
    text: SharedString,
    palette: Palette,
}

impl Render for Hint {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .px(px(9.))
            .py(px(6.))
            .rounded(px(3.))
            .border_1()
            .border_color(rgb(self.palette.border))
            .bg(rgb(self.palette.window))
            .text_size(px(11.))
            .text_color(rgb(self.palette.ink))
            .child(self.text.clone())
    }
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
