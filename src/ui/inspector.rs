// @Time    : 2026/11/5 18:02
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
            .child(self.inspector_header(p, cx))
            .child(self.inspector_body(p))
            .child(
                detail(
                    "点击标题栏可切换会话；拖动窗口边界时 stty size 会同步变化。",
                    p,
                )
                .p(px(20.))
                .line_height(px(18.)),
            )
    }

    fn inspector_header(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
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
            }))
    }

    fn inspector_body(&self, p: &Palette) -> Stateful<Div> {
        let session = self.session();
        div()
            .id("inspector-body")
            .debug_selector(|| "inspector-body".into())
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .p(px(20.))
            .child(info_row("名称", session.title.clone(), p))
            .child(info_row("Shell", session.kind.name(), p))
            .child(info_row("状态", session.state.label(), p))
            .child(info_row(
                "目录",
                self.working_directory().display().to_string(),
                p,
            ))
            .child(info_row("网格", self.grid_size_label(), p))
            .child(info_row("滚动区", self.scrollback_label(), p))
            .child(info_row("可见行", self.visible_lines().to_string(), p))
            .child(self.recent_rows(p))
    }

    /// 右侧最近几行输出。空行不显示，避免滚动区底部留白挤满面板。
    fn recent_rows(&self, p: &Palette) -> Div {
        let session = self.session();
        div()
            .child(
                label("最近输出", p.ink)
                    .text_size(px(13.))
                    .mt(px(28.))
                    .mb(px(12.)),
            )
            .children(
                session
                    .frame
                    .rows
                    .iter()
                    .rev()
                    .filter(|row| row.cells.iter().any(|cell| !cell.text.trim().is_empty()))
                    .take(8)
                    .map(|row| {
                        let text: String =
                            row.cells.iter().map(|cell| cell.text.as_str()).collect();
                        label(text.trim_end().to_string(), p.muted)
                            .font_family(self.font())
                            .text_size(px(12.))
                            .py(px(7.))
                            .truncate()
                    }),
            )
    }

    fn grid_size_label(&self) -> String {
        let session = self.session();
        format!("{} × {}", session.columns, session.rows)
    }

    /// 滚动区大小：网格内存的行数减去可视行即可见的历史行。
    fn scrollback_label(&self) -> String {
        let session = self.session();
        let frame = &session.frame;
        let history = frame.rows.len().saturating_sub(session.rows);
        format!("{history} 行 · 偏移 {}", frame.display_offset)
    }

    fn visible_lines(&self) -> usize {
        self.session()
            .frame
            .rows
            .iter()
            .filter(|row| row.cells.iter().any(|cell| !cell.text.trim().is_empty()))
            .count()
    }

    /// 子进程的 cwd 由 `/proc` 读不到时退回应用自己的目录。
    fn working_directory(&self) -> std::path::PathBuf {
        self.session()
            .pty()
            .and_then(|pty| pty.working_directory())
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| std::path::PathBuf::from("~"))
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
