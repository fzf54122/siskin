// @Time    : 2026/11/5 17:42
// @Author  : fzf
// @FileName: close.rs
// @Software: Siskin

use super::*;

impl SiskinApp {
    pub(super) fn close_confirmation(&self, p: &Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let window = self.close_request == Some(crate::app::CloseRequest::Window);
        div()
            .id("close-confirmation")
            .debug_selector(|| "close-confirmation".into())
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgba(0x00000030))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .w(px(420.))
                    .p(px(24.))
                    .rounded(px(4.))
                    .border_1()
                    .border_color(rgb(p.border))
                    .bg(rgb(p.window))
                    .child(
                        label(
                            if window {
                                "关闭窗口？"
                            } else {
                                "关闭会话？"
                            },
                            p.ink,
                        )
                        .text_size(px(16.))
                        .font_weight(FontWeight::MEDIUM),
                    )
                    .child(
                        detail(
                            "仍有任务在运行。关闭会结束这些进程，未保存的工作可能丢失。",
                            p,
                        )
                        .mt(px(12.))
                        .line_height(px(21.)),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap(px(10.))
                            .mt(px(24.))
                            .child(action("cancel-close", "取消", p, cx, |app| {
                                app.cancel_close()
                            }))
                            .child(
                                action("confirm-close", "结束任务并关闭", p, cx, |app| {
                                    app.confirm_close()
                                })
                                .text_color(rgb(p.error)),
                            ),
                    ),
            )
    }
}
