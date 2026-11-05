// @Time    : 2026/11/4 23:30
// @Author  : fzf
// @FileName: preview.rs
// @Software: Siskin

use super::*;

impl SiskinApp {
    pub(super) fn theme_preview(&self, p: &Palette) -> Stateful<Div> {
        div()
            .id("theme-preview")
            .debug_selector(|| "theme-preview".into())
            .rounded(px(10.))
            .overflow_hidden()
            .border_1()
            .border_color(rgb(p.border))
            .bg(rgb(p.terminal))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .h(px(40.))
                    .px(px(20.))
                    .bg(rgb(p.sidebar))
                    .border_b_1()
                    .border_color(rgb(p.border))
                    .child(detail("终端预览", p))
                    .child(detail(self.settings.theme.name(), p)),
            )
            .child(
                div()
                    .p(px(24.))
                    .font_family(self.font())
                    .text_size(px(self.font_size()))
                    .line_height(px(self.row_height() + 3.))
                    .child(label("~/workspace/siskin", p.path).font_family(self.font()))
                    .child(
                        div()
                            .flex()
                            .gap(px(10.))
                            .mt(px(10.))
                            .child(label("❯", p.prompt))
                            .child(div().text_color(rgb(p.command)).child("cargo check")),
                    )
                    .child(
                        div()
                            .mt(px(8.))
                            .text_color(rgb(p.ok))
                            .child("    Checking siskin v0.1.0"),
                    )
                    .child(
                        div()
                            .text_color(rgb(p.output))
                            .child("    Finished dev profile in 0.42s"),
                    )
                    .child(
                        div()
                            .flex()
                            .gap(px(18.))
                            .mt(px(18.))
                            .child(label("Aa 0123", p.ink))
                            .child(label("中文排版", p.muted))
                            .child(div().w(px(2.)).h(px(17.)).bg(rgb(p.accent))),
                    ),
            )
    }
}
