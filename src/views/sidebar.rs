// @Time    : 2026/11/4 23:30
// @Author  : fzf
// @FileName: sidebar.rs
// @Software: Siskin

use super::*;

impl SiskinApp {
    pub(super) fn session_sidebar(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        div()
            .w(px(208.))
            .flex_shrink_0()
            .h_full()
            .flex()
            .flex_col()
            .bg(rgb(p.sidebar))
            .border_r_1()
            .border_color(rgb(p.border))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .h(px(64.))
                    .px(px(20.))
                    .child(
                        div()
                            .size(px(25.))
                            .rounded(px(6.))
                            .bg(rgb(p.ink))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(rgb(p.window))
                            .text_size(px(12.))
                            .child(">_"),
                    )
                    .child(label("Siskin", p.ink).font_weight(FontWeight::SEMIBOLD)),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px(px(20.))
                    .mb(px(12.))
                    .child(detail("会话", p))
                    .child(
                        action("new-session", "+", p, cx, |app| {
                            app.new_session();
                            app.set_page(Page::Terminal);
                        })
                        .py(px(2.))
                        .px(px(7.)),
                    ),
            )
            .child(
                div()
                    .id("session-list")
                    .debug_selector(|| "session-list".into())
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px(px(10.))
                    .children(
                        self.sessions
                            .iter()
                            .enumerate()
                            .map(|(index, _)| self.session_item(index, p, cx)),
                    ),
            )
            .child(self.sidebar_navigation(p, cx))
    }

    fn session_item(&self, index: usize, p: &Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let selected = self.active == index && self.page == Page::Terminal;
        let session = &self.sessions[index];
        div()
            .id(("session", index))
            .debug_selector(move || format!("session-{index}"))
            .flex()
            .items_center()
            .gap(px(9.))
            .px(px(10.))
            .py(px(11.))
            .mb(px(4.))
            .rounded(px(7.))
            .bg(rgb(if selected { p.window } else { p.sidebar }))
            .cursor_pointer()
            .hover(|style| style.bg(rgb(p.window)))
            .on_click(cx.listener(move |app, _, _, cx| {
                app.select_session(index);
                app.set_page(Page::Terminal);
                cx.notify();
            }))
            .child(label(
                session.kind.prompt_marker(),
                if selected { p.accent } else { p.muted },
            ))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(
                        label(session.title.clone(), p.ink)
                            .text_size(px(13.))
                            .truncate(),
                    )
                    .child(
                        detail(
                            format!("{} · {}", session.kind.name(), session.shell.cwd_label()),
                            p,
                        )
                        .text_size(px(11.))
                        .mt(px(4.)),
                    ),
            )
            .child(self.close_session_button(index, p, cx))
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
            .size(px(20.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(4.))
            .text_color(rgb(p.muted))
            .text_size(px(14.))
            .cursor_pointer()
            .hover(|style| style.bg(rgb(p.tint)))
            .on_click(cx.listener(move |app, _, _, cx| {
                app.close_session(index);
                cx.stop_propagation();
                cx.notify();
            }))
            .child("×")
    }

    fn sidebar_navigation(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        div()
            .p(px(12.))
            .border_t_1()
            .border_color(rgb(p.border))
            .child(
                action("nav-terminal", "终端工作区", p, cx, |app| {
                    app.set_page(Page::Terminal)
                })
                .w_full()
                .mb(px(6.)),
            )
            .child(
                action("nav-settings", "偏好设置", p, cx, |app| {
                    app.set_page(Page::Settings)
                })
                .w_full()
                .when(self.page == Page::Settings, |view| {
                    view.bg(rgb(p.tint))
                        .text_color(rgb(p.accent))
                        .border_color(rgb(p.tint))
                }),
            )
    }
}
