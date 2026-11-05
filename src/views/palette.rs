// @Time    : 2026/11/4 23:30
// @Author  : fzf
// @FileName: palette.rs
// @Software: Siskin

use super::*;

impl SiskinApp {
    pub(super) fn command_palette(&self, p: &Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let query = self
            .palette
            .as_ref()
            .map(|value| value.query.clone())
            .unwrap_or_default();
        div()
            .id("palette")
            .debug_selector(|| "palette".into())
            .absolute()
            .inset_0()
            .flex()
            .flex_col()
            .items_center()
            .pt(px(80.))
            .bg(rgba(0x0b102055))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|app, _, _, cx| {
                    app.close_palette();
                    cx.notify();
                }),
            )
            .child(
                div()
                    .w(px(580.))
                    .max_w_full()
                    .rounded(px(12.))
                    .shadow_lg()
                    .bg(rgb(p.window))
                    .border_1()
                    .border_color(rgb(p.border))
                    .overflow_hidden()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(12.))
                            .p(px(18.))
                            .border_b_1()
                            .border_color(rgb(p.border))
                            .child(label("›", p.accent).text_size(px(20.)))
                            .child(
                                label(
                                    if query.is_empty() {
                                        "搜索命令…".into()
                                    } else {
                                        query
                                    },
                                    p.ink,
                                )
                                .flex_1(),
                            )
                            .child(key_hint("Esc", p)),
                    )
                    .child(
                        div()
                            .id("palette-results")
                            .max_h(px(440.))
                            .overflow_y_scroll()
                            .p(px(6.))
                            .children(
                                self.palette_entries()
                                    .into_iter()
                                    .enumerate()
                                    .map(|(index, entry)| self.palette_result(index, entry, p, cx)),
                            ),
                    ),
            )
    }

    fn palette_result(
        &self,
        index: usize,
        entry: &'static crate::app::PaletteEntry,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let selected = self
            .palette
            .as_ref()
            .is_some_and(|value| value.selected == index);
        let action = entry.action;
        div()
            .id(("palette-entry", index))
            .debug_selector(move || format!("palette-{index}"))
            .flex()
            .items_center()
            .justify_between()
            .p(px(12.))
            .rounded(px(7.))
            .bg(rgb(if selected { p.tint } else { p.window }))
            .cursor_pointer()
            .hover(|style| style.bg(rgb(p.sidebar)))
            .on_click(cx.listener(move |app, _, _, cx| {
                app.close_palette();
                app.run_palette_action(action);
                cx.notify();
            }))
            .child(
                div()
                    .child(label(entry.label, p.ink))
                    .child(detail(entry.hint, p).mt(px(4.))),
            )
            .child(detail(entry.keys, p))
    }
}
