// @Time    : 2026/11/4 23:30
// @Author  : fzf
// @FileName: workspace.rs
// @Software: Siskin

use gpui::{CursorStyle, MouseDownEvent};

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

    /// 按分屏树递归生成终端区。叶按 `resize_panes` 算出的矩形定尺寸
    /// （不用 `flex_1` 均分，否则比例会丢），分隔线是 4px 的固定条，
    /// 按下即开始拖动（`start_pane_drag`），移动与松开挂在页根上。
    pub(super) fn pane_node(
        &self,
        tree: &crate::app::pane::PaneTree,
        rect: gpui::Bounds<gpui::Pixels>,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Div {
        match tree {
            crate::app::pane::PaneTree::Leaf(id) => div()
                .w(rect.size.width)
                .h(rect.size.height)
                .flex_shrink_0()
                .flex()
                .flex_col()
                .child(self.terminal_body(*id, p, cx)),
            crate::app::pane::PaneTree::Split {
                direction,
                ratio,
                first,
                second,
            } => {
                let (first_rect, second_rect) =
                    crate::app::pane::PaneTree::split_rects(rect, *direction, *ratio);
                let second_leaf = second.first_leaf().unwrap_or(0);
                let horizontal = *direction == crate::app::pane::SplitDirection::Horizontal;
                let divider = div()
                    .id(("pane-divider", second_leaf))
                    .debug_selector(|| "pane-divider".into())
                    .flex_shrink_0()
                    .bg(rgb(p.border))
                    .cursor(if horizontal {
                        CursorStyle::ResizeUpDown
                    } else {
                        CursorStyle::ResizeLeftRight
                    })
                    .when(horizontal, |view| {
                        view.w_full().h(px(crate::app::pane::PANE_DIVIDER))
                    })
                    .when(!horizontal, |view| {
                        view.h_full().w(px(crate::app::pane::PANE_DIVIDER))
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |app, event: &MouseDownEvent, _, cx| {
                            app.start_pane_drag(second_leaf, event.position);
                            cx.notify();
                            cx.stop_propagation();
                        }),
                    );
                let container = div()
                    .w(rect.size.width)
                    .h(rect.size.height)
                    .flex_shrink_0()
                    .flex()
                    .when(horizontal, |view| view.flex_col());
                container
                    .child(self.pane_node(first, first_rect, p, cx))
                    .child(divider)
                    .child(self.pane_node(second, second_rect, p, cx))
            }
        }
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
