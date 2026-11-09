// @Time    : 2026/11/4 23:30
// @Author  : fzf
// @FileName: workspace.rs
// @Software: Siskin

use super::*;

impl SiskinApp {
    pub(super) fn workspace(&self, p: &Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        div()
            .id("workspace")
            .debug_selector(|| "workspace".into())
            .relative()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .when(self.page == Page::Terminal, |view| {
                view.child(self.session_tabs(p, cx))
                    .child(div().flex_1().min_h_0().child(self.terminal_page(p, cx)))
            })
            .when(self.page == Page::Settings, |view| {
                view.child(self.settings_page(p, cx))
            })
            .when(self.status.is_some(), |view| {
                view.child(self.status_banner(p, cx))
            })
    }

    fn status_banner(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        div()
            .absolute()
            .bottom(px(16.))
            .left(px(16.))
            .right(px(16.))
            .p(px(12.))
            .rounded(px(3.))
            .border_1()
            .border_color(rgb(p.error))
            .bg(rgb(p.window))
            .flex()
            .items_center()
            .gap(px(12.))
            .child(
                label(self.status.clone().unwrap_or_default(), p.error)
                    .text_size(px(12.))
                    .flex_1(),
            )
            .child(chrome_action("dismiss-status", "×", p, cx, |app| {
                app.status = None
            }))
    }
}
