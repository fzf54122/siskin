// @Time    : 2026/11/4 23:30
// @Author  : fzf
// @FileName: inspector.rs
// @Software: Siskin

use super::*;

impl SiskinApp {
    pub(super) fn session_inspector(&self, p: &Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        div()
            .id("session-inspector")
            .debug_selector(|| "session-inspector".into())
            .w(px(280.))
            .flex_shrink_0()
            .h_full()
            .flex()
            .flex_col()
            .bg(rgb(p.window))
            .border_l_1()
            .border_color(rgb(p.border))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .h(px(56.))
                    .px(px(20.))
                    .border_b_1()
                    .border_color(rgb(p.border))
                    .child(
                        label("会话信息", p.ink)
                            .text_size(px(13.))
                            .font_weight(FontWeight::MEDIUM),
                    )
                    .child(action("close-inspector", "×", p, cx, |app| {
                        app.inspector_open = false
                    })),
            )
            .child(
                div()
                    .id("session-inspector")
                    .debug_selector(|| "session-inspector".into())
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p(px(20.))
                    .child(info_row("名称", self.session().title.clone(), p))
                    .child(info_row("Shell", self.session().kind.name(), p))
                    .child(info_row("目录", self.session().shell.cwd_label(), p))
                    .child(info_row("输出行", self.line_count().to_string(), p))
                    .child(info_row("历史命令", self.command_count().to_string(), p))
                    .child(
                        label("最近使用", p.ink)
                            .text_size(px(13.))
                            .mt(px(28.))
                            .mb(px(12.)),
                    )
                    .children(
                        self.session()
                            .shell
                            .history()
                            .iter()
                            .rev()
                            .take(8)
                            .map(|command| {
                                label(command.clone(), p.muted)
                                    .font_family(self.font())
                                    .text_size(px(12.))
                                    .py(px(7.))
                                    .truncate()
                            }),
                    )
                    .when(self.command_count() == 0, |view| {
                        view.child(detail("还没有命令记录。", p))
                    }),
            )
            .child(
                detail("当前为界面预览，未连接真实 PTY。", p)
                    .p(px(20.))
                    .line_height(px(18.)),
            )
    }
}

fn info_row(title: &'static str, value: impl Into<SharedString>, p: &Palette) -> Div {
    div()
        .py(px(10.))
        .border_b_1()
        .border_color(rgb(p.hairline))
        .child(detail(title, p))
        .child(label(value, p.ink).text_size(px(13.)).mt(px(5.)))
}
