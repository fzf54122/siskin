// @Time    : 2026/11/4 23:30
// @Author  : fzf
// @FileName: workspace.rs
// @Software: Siskin

use super::*;

impl SiskinApp {
    pub(super) fn workspace(&self, p: &Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let content = match self.page {
            Page::Terminal => self.terminal_page(p, cx).into_any_element(),
            Page::Settings => self.settings_page(p, cx).into_any_element(),
        };
        div()
            .id("workspace")
            .debug_selector(|| "workspace".into())
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .child(self.workspace_heading(p, cx))
            .child(div().flex_1().min_h_0().child(content))
    }

    fn workspace_heading(&self, p: &Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let title = if self.page == Page::Settings {
            "偏好设置".into()
        } else {
            self.session().title.clone()
        };
        div()
            .id("workspace-heading")
            .debug_selector(|| "workspace-heading".into())
            .flex()
            .items_center()
            .justify_between()
            .h(px(56.))
            .px(px(24.))
            .flex_shrink_0()
            .border_b_1()
            .border_color(rgb(p.border))
            .bg(rgb(p.window))
            .child(
                label(title, p.ink)
                    .text_size(px(13.))
                    .font_weight(FontWeight::MEDIUM),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(action("open-commands", "命令  Ctrl K", p, cx, |app| {
                        app.open_palette()
                    }))
                    .when(self.page == Page::Terminal, |view| {
                        view.child(
                            action("toggle-inspector", "会话信息", p, cx, |app| {
                                app.inspector_open = !app.inspector_open;
                            })
                            .bg(rgb(if self.inspector_open {
                                p.tint
                            } else {
                                p.window
                            })),
                        )
                    }),
            )
    }
}
