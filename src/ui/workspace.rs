// @Time    : 2026/11/4 23:30
// @Author  : fzf
// @FileName: workspace.rs
// @Software: Siskin

use super::*;
use crate::domain::Action;

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
                view.child(self.workspace_heading(p, cx))
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

    fn workspace_heading(&self, p: &Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let path = self.directory_label();
        let title = if path.chars().count() > 64 {
            self.session()
                .directory
                .as_ref()
                .and_then(|directory| directory.file_name())
                .map_or(path.clone(), |name| format!("…/{}", name.to_string_lossy()))
        } else {
            path
        };
        div()
            .id("workspace-heading")
            .debug_selector(|| "workspace-heading".into())
            .flex()
            .items_center()
            .gap(px(8.))
            .h(px(HEADING_HEIGHT))
            .px(px(20.))
            .flex_shrink_0()
            .border_b_1()
            .border_color(rgb(p.hairline))
            .bg(rgb(p.window))
            .child(chrome_icon(ChromeIcon::Folder, p.muted))
            .child(
                label(title, p.muted)
                    .text_size(px(12.))
                    .flex_1()
                    .min_w_0()
                    .truncate(),
            )
            .child(icon_action(
                "open-search",
                ChromeIcon::Search,
                "搜索终端 · Ctrl+Shift+F",
                self.search.is_some(),
                Action::OpenSearch,
                p,
                cx,
            ))
            .child(icon_action(
                "toggle-inspector",
                ChromeIcon::Panel,
                "会话信息 · Ctrl+Shift+I",
                self.inspector_open,
                Action::ToggleInspector,
                p,
                cx,
            ))
    }
}
