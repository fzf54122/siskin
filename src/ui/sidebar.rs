// @Time    : 2026/11/5 18:00
// @Author  : fzf
// @FileName: sidebar.rs
// @Software: Siskin

use super::brand::brand_mark;
use super::*;
use crate::domain::{Action, SessionState};

impl SiskinApp {
    pub(super) fn session_sidebar(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        if self.page == Page::Settings {
            return self.settings_sidebar(p, cx);
        }
        side_rail(p)
            .child(
                div()
                    .h(px(HEADING_HEIGHT))
                    .flex_shrink_0()
                    .px(px(14.))
                    .flex()
                    .items_center()
                    .gap(px(9.))
                    .child(brand_mark(24.))
                    .child(
                        label("会话", p.ink)
                            .text_size(px(13.))
                            .font_weight(FontWeight::MEDIUM),
                    )
                    .child(detail(self.sessions.len().to_string(), p).text_size(px(10.)))
                    .child(div().flex_1())
                    .child(icon_action(
                        "new-session",
                        ChromeIcon::Plus,
                        "新建会话 · Ctrl+Shift+N",
                        false,
                        Action::NewSession,
                        p,
                        cx,
                    )),
            )
            .child(
                div()
                    .id("session-list")
                    .debug_selector(|| "session-list".into())
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.session_scroll)
                    .px(px(12.))
                    .pt(px(11.))
                    .flex()
                    .flex_col()
                    .gap(px(5.))
                    .children(
                        self.sessions
                            .iter()
                            .map(|(index, _)| self.session_item(index, p, cx)),
                    ),
            )
            .child(
                div()
                    .mx(px(12.))
                    .pb(px(12.))
                    .pt(px(10.))
                    .border_t_1()
                    .border_color(rgb(p.hairline))
                    .child(side_action(
                        "nav-settings",
                        "设置",
                        "Ctrl ,",
                        ChromeIcon::Palette,
                        p,
                        cx,
                        |app| {
                            app.dispatch(Action::OpenSettings);
                        },
                    )),
            )
    }

    fn session_item(&self, index: usize, p: &Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let Some(session) = self.sessions.get(index) else {
            return div().id(("session-missing", index));
        };
        let selected = index == self.active_index();
        div()
            .id(("session", index))
            .debug_selector(move || format!("session-{index}"))
            .relative()
            .h(px(40.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap(px(9.))
            .px(px(10.))
            .rounded(px(3.))
            .border_1()
            .border_color(rgb(if selected { p.border } else { p.sidebar }))
            .bg(rgb(p.sidebar))
            .cursor_pointer()
            .hover(|style| style.border_color(rgb(p.border)))
            .on_click(cx.listener(move |app, _, _, cx| {
                app.select_session(index);
                app.set_page(Page::Terminal);
                cx.notify();
            }))
            .when(selected, |view| {
                view.child(
                    div()
                        .absolute()
                        .left(px(0.))
                        .top(px(6.))
                        .bottom(px(6.))
                        .w(px(3.))
                        .bg(rgb(p.ink)),
                )
            })
            .child(chrome_icon(
                ChromeIcon::Terminal,
                if selected { p.ink } else { p.muted },
            ))
            .child(
                label(
                    session.display_title(),
                    if selected { p.ink } else { p.muted },
                )
                .text_size(px(13.))
                .flex_1()
                .min_w_0()
                .truncate()
                .when(selected, |view| view.font_weight(FontWeight::MEDIUM)),
            )
            .when(matches!(session.state, SessionState::Failed(_)), |view| {
                view.child(div().size(px(4.)).rounded_full().bg(rgb(p.error)))
            })
            .child(
                self.close_session_button(index, p, cx)
                    .opacity(if selected { 0.8 } else { 0.45 }),
            )
    }

    fn close_session_button(
        &self,
        index: usize,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        div()
            .id(("close-session", index))
            .debug_selector(move || format!("close-session-{index}"))
            .size(px(24.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(3.))
            .cursor_pointer()
            .hover(|style| style.bg(rgb(p.tint)))
            .on_click(cx.listener(move |app, _, _, cx| {
                app.close_session(index);
                cx.stop_propagation();
                cx.notify();
            }))
            .child(chrome_icon(ChromeIcon::Close, p.faint).size(px(12.)))
    }
}

pub(super) fn side_rail(p: &Palette) -> Div {
    div()
        .w(px(SIDEBAR_WIDTH))
        .flex_shrink_0()
        .h_full()
        .flex()
        .flex_col()
        .bg(rgb(p.sidebar))
        .border_r_1()
        .border_color(rgb(p.border))
}

pub(super) fn side_action(
    id: &'static str,
    title: &'static str,
    keys: &'static str,
    icon: ChromeIcon,
    p: &Palette,
    cx: &mut Context<SiskinApp>,
    callback: impl Fn(&mut SiskinApp) + 'static,
) -> Stateful<Div> {
    chrome_action(id, "", p, cx, callback)
        .w_full()
        .h(px(35.))
        .gap(px(10.))
        .justify_start()
        .child(chrome_icon(icon, p.muted))
        .child(label(title, p.ink).text_size(px(12.)))
        .child(div().flex_1())
        .child(detail(keys, p).text_size(px(10.)))
}
