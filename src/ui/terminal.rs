// @Time    : 2026/11/5 17:44
// @Author  : fzf
// @FileName: terminal.rs
// @Software: Siskin

//! 终端卡片：网格、滚动、鼠标选区与输入法锚点。
//!
//! 鼠标优先级与常见终端一致：应用开启鼠标上报时转成 SGR 序列，
//! 否则本地拖选；本地拖选结束把文本写入 PRIMARY。

use super::grid::{CursorSlot, GridInputs};
use super::*;
use crate::io::search::MatchSpan;
use gpui::{CursorStyle, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ScrollEvent};
use std::sync::Arc;

impl SiskinApp {
    /// 终端页：按活动标签的分屏树递归生成多个 `terminal_body`，
    /// 单叶时与以前一样只是一张卡片。分隔线拖动挂在页根上，
    /// 指针离开分隔线本身仍能收到移动事件，松开即结束。
    pub(super) fn terminal_page(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        let tree = self.active_tab_tree();
        let area = self.pane_area;
        div()
            .size_full()
            .flex()
            .flex_col()
            .on_mouse_move(cx.listener(|app, event: &MouseMoveEvent, _, cx| {
                if app.scrollbar_drag.is_some() {
                    if app.update_scrollbar_drag(event.position.y.as_f32()) {
                        cx.notify();
                    }
                    return;
                }
                if app.pane_drag.is_some() {
                    app.update_pane_drag(event.position);
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|app, _: &MouseUpEvent, _, cx| {
                    if app.end_scrollbar_drag() {
                        cx.notify();
                        return;
                    }
                    if app.pane_drag.is_some() {
                        app.end_pane_drag();
                        cx.notify();
                    }
                }),
            )
            .child(self.pane_node(&tree, area, p, cx))
    }

    fn cursor_slot(&self) -> CursorSlot {
        Arc::clone(&self.cursor_slot)
    }

    /// 一张终端卡片，现在按会话 id 参数化：分屏后每叶一张，
    /// 帧、滚动、选区、搜索高亮都取该叶自己的会话。搜索条只画在
    /// 焦点 pane 上，光标也只在焦点 pane 里是实色。
    pub(super) fn terminal_body(
        &self,
        session_id: u64,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let Some(session) = self.session_by_id(session_id) else {
            return div().id(("terminal-card", session_id)).flex_1().min_h_0();
        };
        let scrolled = session.frame.display_offset > 0;
        let focused = session_id == self.active_pane;
        div()
            .id(("terminal-card", session_id))
            .debug_selector(|| "terminal-card".into())
            .relative()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .bg(rgb(p.terminal))
            .child(self.mouse_target(session_id, cx))
            .child(self.grid_layer(session_id, p, scrolled, cx))
            .when(focused && self.search.is_some(), |view| {
                view.child(self.search_bar(p, cx))
            })
    }

    /// 搜索条浮在左上角。匹配数量与当前位置直接来自 `SearchState::label`。
    fn search_bar(&self, p: &Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let text = self
            .search
            .as_ref()
            .map(|state| state.label())
            .unwrap_or_else(|| "搜索".to_string());
        div()
            .id("search-bar")
            .debug_selector(|| "search-bar".into())
            .absolute()
            .top(px(12.))
            .right(px(20.))
            .flex()
            .items_center()
            .gap(px(10.))
            .px(px(12.))
            .py(px(8.))
            .rounded(px(7.))
            .bg(rgb(p.window))
            .border_1()
            .border_color(rgb(p.border))
            .text_size(px(12.))
            .text_color(rgb(p.ink))
            .child(label("›", p.accent))
            .child(label(text, p.ink))
            .child(self.search_toggle("search-case", "Aa", p, cx, true))
            .child(self.search_toggle("search-regex", ".*", p, cx, false))
            .child(
                div()
                    .id("search-next")
                    .debug_selector(|| "search-next".into())
                    .px(px(6.))
                    .cursor_pointer()
                    .on_click(cx.listener(|app, _, _, cx| {
                        app.step_match(true);
                        cx.notify();
                    }))
                    .child("↓"),
            )
            .child(
                div()
                    .id("search-close")
                    .debug_selector(|| "search-close".into())
                    .px(px(6.))
                    .cursor_pointer()
                    .on_click(cx.listener(|app, _, _, cx| {
                        app.close_search();
                        cx.notify();
                    }))
                    .child("×"),
            )
    }

    /// 当前帧的搜索高亮（绝对行换算成可见行），只画在焦点 pane 上。
    /// 注意这里不能复用 `grid_layer` 里的 `focused`：搜索条打开时它
    /// 为假（光标要变暗），但高亮必须照常显示，否则边打字边看不到匹配。
    fn search_spans(&self, session_id: u64) -> (Vec<MatchSpan>, Option<MatchSpan>) {
        if session_id != self.active_pane {
            return (Vec::new(), None);
        }
        let Some(session) = self.session_by_id(session_id) else {
            return (Vec::new(), None);
        };
        let Some(state) = self.search.as_ref() else {
            return (Vec::new(), None);
        };
        let offset = session.frame.display_offset;
        let rows = session.frame.rows.len();
        (
            state.visible_spans(offset, rows),
            state.current_visible_span(offset, rows),
        )
    }

    /// 搜索条里的大小写 / 正则开关。激活时用强调色，点一下即切换并重算。
    fn search_toggle(
        &self,
        id: &'static str,
        text: &'static str,
        p: &Palette,
        cx: &mut Context<Self>,
        is_case: bool,
    ) -> Stateful<Div> {
        let active = self
            .search
            .as_ref()
            .map(|state| {
                if is_case {
                    state.case_sensitive
                } else {
                    state.regex
                }
            })
            .unwrap_or(false);
        div()
            .id(id)
            .debug_selector(move || id.into())
            .px(px(6.))
            .cursor_pointer()
            .text_color(rgb(if active { p.accent } else { p.ink }))
            .on_click(cx.listener(move |app, _, _, cx| {
                if is_case {
                    app.toggle_search_case();
                } else {
                    app.toggle_search_regex();
                }
                cx.notify();
            }))
            .child(text)
    }

    /// 鼠标事件挂在整张卡片上：网格铺满卡片，坐标换算的基准也是卡片原点。
    /// 每个回调都先按本叶 id 换算行列；左键按下会先把焦点切到本叶，
    /// 再走原来的链接 → 鼠标上报 → 本地拖选顺序。
    fn mouse_target(&self, session_id: u64, cx: &mut Context<Self>) -> Stateful<Div> {
        div()
            .id(("terminal-mouse", session_id))
            .absolute()
            .inset_0()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |app, event: &MouseDownEvent, window, cx| {
                    // 先拿焦点，再处理点击，否则 `Ctrl+Click` 后输入法仍指向别的控件。
                    // 点进终端也是重命名编辑框失焦：先提交，再走点击。
                    app.commit_tab_rename();
                    app.focus.focus(window, cx);
                    app.press_left(session_id, event);
                    cx.notify();
                }),
            )
            .on_mouse_down(
                MouseButton::Middle,
                cx.listener(move |app, event: &MouseDownEvent, _, cx| {
                    let (row, column) = app.cell_at(session_id, event.position);
                    if !app.forward_mouse(session_id, row, column, 1, true, false) {
                        app.paste_primary(cx);
                    }
                    cx.notify();
                }),
            )
            .on_mouse_up(
                MouseButton::Right,
                cx.listener(move |app, event: &MouseUpEvent, _, cx| {
                    let (row, column) = app.cell_at(session_id, event.position);
                    app.forward_mouse(session_id, row, column, 2, false, event.modifiers.shift);
                    cx.notify();
                }),
            )
            .on_mouse_move(cx.listener(move |app, event: &MouseMoveEvent, _, cx| {
                // 无变化的纯移动不通知：拖选端点没动格、鼠标上报没发出时，
                // 重绘只会白跑一帧。上报路径本身不受影响。
                if app.drag_to(session_id, event.position) {
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |app, _: &MouseUpEvent, _, cx| {
                    app.release_left(session_id);
                    cx.notify();
                }),
            )
            .on_scroll(cx.listener(move |app, event: &ScrollEvent, _, cx| {
                app.scroll_wheel(session_id, event);
                cx.notify();
                cx.stop_propagation();
            }))
    }

    fn grid_layer(
        &self,
        session_id: u64,
        p: &Palette,
        scrolled: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        // 亚行偏移：滚轮位移里不足一行的部分，渲染时整帧平移到这个位置。
        // 鼠标换算用同一个值补偿，点击不会错行。
        let Some(session) = self.session_by_id(session_id) else {
            return div().id(("terminal-grid", session_id)).flex_1();
        };
        let sub = session.sub_pixel_offset(self.row_height());
        let focused = session_id == self.active_pane
            && self.page == Page::Terminal
            && self.palette.is_none()
            && self.search.is_none()
            && self.close_request.is_none();
        // 只有焦点 pane 写共享的光标/原点槽：非焦点 pane 写进去会
        // 抢走输入法候选框锚点，丢弃的槽每帧新建一次即可。
        let (cursor_slot, origin_slot) = if focused {
            (self.cursor_slot(), Arc::clone(&self.grid_origin))
        } else {
            (
                Arc::new(std::sync::Mutex::new(None)),
                Arc::new(std::sync::Mutex::new(None)),
            )
        };
        let (highlights, current_match) = self.search_spans(session_id);
        div()
            .id(("terminal-grid", session_id))
            .debug_selector(|| "terminal-grid".into())
            .relative()
            .flex_1()
            .min_h_0()
            .overflow_hidden()
            .cursor(CursorStyle::IBeam)
            .child(
                GridInputs {
                    frame: session.frame_snapshot(),
                    offset_y: sub,
                    metrics: self.metrics,
                    font: self.font(),
                    palette_bg: p.terminal,
                    palette_fg: p.ink,
                    selection_bg: p.tint,
                    highlight_bg: p.warn,
                    match_bg: p.warn,
                    highlights,
                    current_match,
                    cursor: p.accent,
                    link: p.path,
                    focused,
                    cursor_slot,
                    origin_slot,
                    view: cx.entity(),
                    focus: self.focus.clone(),
                }
                .into_element()
                .debug_selector("terminal-grid-canvas"),
            )
            .when(scrolled, |view| {
                view.child(self.scroll_bottom_button(session_id, p, cx))
            })
            .when_some(self.scrollbar(session_id, p, cx), |view, bar| {
                view.child(bar)
            })
    }

    fn scroll_bottom_button(
        &self,
        session_id: u64,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        div()
            .id(("scroll-bottom", session_id))
            .debug_selector(|| "scroll-bottom".into())
            .absolute()
            .bottom(px(16.))
            .right(px(24.))
            .px(px(10.))
            .py(px(7.))
            .rounded(px(6.))
            .bg(rgb(p.window))
            .border_1()
            .border_color(rgb(p.border))
            .text_size(px(12.))
            .text_color(rgb(p.ink))
            .cursor_pointer()
            .on_click(cx.listener(move |app, _, _, cx| {
                app.focus_pane(session_id);
                app.scroll_to_bottom();
                let colors = app.ansi();
                app.session_mut().refresh(&colors);
                cx.notify();
            }))
            .child("回到底部 ↓")
    }
}
