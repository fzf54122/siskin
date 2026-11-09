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
            .rounded(px(3.))
            .border_1()
            .border_color(rgb(p.hairline))
            .bg(rgb(p.terminal))
            .p(px(20.))
            .child(
                detail("字形预览", p)
                    .font_family(self.ui.clone())
                    .text_size(px(11.))
                    .mb(px(14.)),
            )
            .child(
                div()
                    .font_family(self.font())
                    .text_size(px(self.font_size()))
                    .line_height(px(self.row_height()))
                    .child(div().child("Aa Bb Cc  0123456789"))
                    .child(div().child("Il1 O0 {} [] () => !="))
                    .child(div().child("中文输入 · 对齐与间距")),
            )
    }
}
