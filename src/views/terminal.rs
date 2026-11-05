// @Time    : 2026/11/4 23:30
// @Author  : fzf
// @FileName: terminal.rs
// @Software: Siskin

use super::*;

impl SiskinApp {
    pub(super) fn terminal_page(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(self.terminal_body(p, cx))
    }

    fn terminal_body(&self, p: &Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let rows = self.session().terminal.wrapped();
        div()
            .id("terminal-card")
            .debug_selector(|| "terminal-card".into())
            .relative()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .bg(rgb(p.terminal))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|app, _, window, cx| {
                    app.focus.focus(window, cx);
                }),
            )
            .child(
                div()
                    .id("scrollback")
                    .debug_selector(|| "scrollback".into())
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.session().scroll)
                    .px(px(28.))
                    .pt(px(24.))
                    .pb(px(12.))
                    .font_family(self.font())
                    .when(rows.is_empty(), |view| view.child(self.welcome(p)))
                    .children(
                        rows.into_iter()
                            .map(|line| render_line(line, p, self.font_size(), self.row_height())),
                    ),
            )
            .when(self.session().terminal.scroll() > 0, |view| {
                view.child(
                    div()
                        .id("scroll-bottom")
                        .debug_selector(|| "scroll-bottom".into())
                        .absolute()
                        .bottom(px(16.))
                        .right(px(24.))
                        .px(px(10.))
                        .py(px(7.))
                        .rounded(px(6.))
                        .bg(rgb(p.window))
                        .border_1()
                        .border_color(rgb(p.border))
                        .text_size(px(12.))
                        .text_color(rgb(p.ink))
                        .cursor_pointer()
                        .on_click(cx.listener(|app, _, _, cx| {
                            let session = app.session_mut();
                            session.terminal.scroll_to_bottom();
                            session.scroll.scroll_to_bottom();
                            cx.notify();
                        }))
                        .child("回到底部 ↓"),
                )
            })
            .child(self.terminal_input(p))
    }

    fn welcome(&self, p: &Palette) -> Div {
        div()
            .mb(px(24.))
            .font_family(self.ui.clone())
            .child(
                label("Siskin", p.ink)
                    .text_size(px(21.))
                    .font_weight(FontWeight::SEMIBOLD),
            )
            .child(detail("一个安静、专注的终端工作空间。", p).mt(px(8.)))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .mt(px(18.))
                    .child(key_hint("Ctrl K", p))
                    .child(detail("命令面板", p))
                    .child(key_hint("Ctrl ,", p).ml(px(12.)))
                    .child(detail("偏好设置", p)),
            )
            .child(detail("当前为界面预览，输入 help 可查看演示命令。", p).mt(px(20.)))
    }

    fn terminal_input(&self, p: &Palette) -> Stateful<Div> {
        div()
            .id("terminal-input")
            .debug_selector(|| "terminal-input".into())
            .flex()
            .items_center()
            .gap(px(10.))
            .px(px(28.))
            .py(px(16.))
            .border_t_1()
            .border_color(rgb(p.hairline))
            .flex_shrink_0()
            .font_family(self.font())
            .text_size(px(self.font_size()))
            .child(label(self.session().shell.cwd_label(), p.path))
            .child(label(self.session().kind.prompt_marker(), p.prompt))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .text_color(rgb(p.command))
                    .child(self.session().terminal.input().to_string())
                    .child(
                        div()
                            .id("input-caret")
                            .debug_selector(|| "input-caret".into())
                            .w(px(2.))
                            .h(px(self.font_size() + 4.))
                            .bg(rgb(p.accent)),
                    ),
            )
    }
}
