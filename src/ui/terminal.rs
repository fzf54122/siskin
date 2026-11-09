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
use gpui::{CursorStyle, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ScrollDelta, ScrollEvent};
use std::sync::Arc;

impl SiskinApp {
    pub(super) fn terminal_page(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(self.terminal_body(p, cx))
    }

    fn cursor_slot(&self) -> CursorSlot {
        Arc::clone(&self.cursor_slot)
    }

    fn terminal_body(&self, p: &Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let scrolled = self.session().frame.display_offset > 0;
        div()
            .id("terminal-card")
            .debug_selector(|| "terminal-card".into())
            .relative()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .bg(rgb(p.terminal))
            .child(self.mouse_target(cx))
            .child(self.grid_layer(p, scrolled, cx))
            .when(self.search.is_some(), |view| {
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

    /// 鼠标事件挂在整张卡片上：网格铺满卡片，坐标换算的基准也是卡片原点。
    fn mouse_target(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        div()
            .id("terminal-mouse")
            .absolute()
            .inset_0()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|app, event: &MouseDownEvent, window, cx| {
                    // 先拿焦点，再处理点击，否则 `Ctrl+Click` 后输入法仍指向别的控件。
                    app.focus.focus(window, cx);
                    app.press_left(event);
                    cx.notify();
                }),
            )
            .on_mouse_down(
                MouseButton::Middle,
                cx.listener(|app, event: &MouseDownEvent, _, cx| {
                    let (row, column) = app.cell_at(event.position);
                    if !app.forward_mouse(row, column, 1, true, false) {
                        app.paste_primary(cx);
                    }
                    cx.notify();
                }),
            )
            .on_mouse_up(
                MouseButton::Right,
                cx.listener(|app, event: &MouseUpEvent, _, cx| {
                    let (row, column) = app.cell_at(event.position);
                    app.forward_mouse(row, column, 2, false, event.modifiers.shift);
                    cx.notify();
                }),
            )
            .on_mouse_move(cx.listener(|app, event: &MouseMoveEvent, _, cx| {
                app.drag_to(event.position);
                cx.notify();
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|app, _: &MouseUpEvent, _, cx| {
                    app.release_left();
                    cx.notify();
                }),
            )
            .on_scroll(cx.listener(|app, event: &ScrollEvent, _, cx| {
                app.scroll_wheel(event);
                cx.notify();
                cx.stop_propagation();
            }))
    }

    fn grid_layer(&self, p: &Palette, scrolled: bool, cx: &mut Context<Self>) -> Stateful<Div> {
        div()
            .id("terminal-grid")
            .debug_selector(|| "terminal-grid".into())
            .relative()
            .flex_1()
            .min_h_0()
            .overflow_hidden()
            .cursor(CursorStyle::IBeam)
            .child(
                GridInputs {
                    frame: self.session().frame.clone(),
                    metrics: self.metrics,
                    font: self.font(),
                    palette_bg: p.terminal,
                    palette_fg: p.ink,
                    selection_bg: p.tint,
                    highlight_bg: p.warn,
                    match_bg: p.warn,
                    highlights: self
                        .search
                        .as_ref()
                        .map(|state| state.matches.clone())
                        .unwrap_or_default(),
                    current_match: self.search.as_ref().and_then(|state| state.current_span()),
                    cursor: p.accent,
                    link: p.path,
                    focused: self.page == Page::Terminal
                        && self.palette.is_none()
                        && self.search.is_none()
                        && self.close_request.is_none(),
                    cursor_slot: self.cursor_slot(),
                    origin_slot: Arc::clone(&self.grid_origin),
                    view: cx.entity(),
                    focus: self.focus.clone(),
                }
                .into_element()
                .debug_selector("terminal-grid-canvas"),
            )
            .when(scrolled, |view| {
                view.child(self.scroll_bottom_button(p, cx))
            })
    }

    fn scroll_bottom_button(&self, p: &Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        div()
            .id("scroll-bottom")
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
            .on_click(cx.listener(|app, _, _, cx| {
                app.scroll_to_bottom();
                let colors = app.ansi();
                app.session_mut().refresh(&colors);
                cx.notify();
            }))
            .child("回到底部 ↓")
    }

    /// 左键按下：`Ctrl+Click` 打开链接 → 鼠标上报 → 本地拖选。
    fn press_left(&mut self, event: &MouseDownEvent) {
        let (row, column) = self.cell_at(event.position);
        if event.modifiers.secondary() && self.open_link_at(row, column) {
            return;
        }
        if self.forward_mouse(row, column, 0, true, event.modifiers.shift) {
            return;
        }
        self.drag = Some((row as f32, column as f32));
        self.session_mut().clear_selection();
        if let Some(pty) = self.session_mut().pty_mut() {
            pty.select(row, column);
        }
    }

    /// 拖动中延长选区；没有按下时把移动事件转成鼠标上报。
    fn drag_to(&mut self, position: gpui::Point<gpui::Pixels>) {
        let (row, column) = self.cell_at(position);
        if self.drag.is_some() {
            if let Some(pty) = self.session_mut().pty_mut() {
                pty.update_selection(row, column);
            }
            return;
        }
        self.forward_mouse(row, column, 3, true, false);
    }

    fn release_left(&mut self) {
        if self.drag.take().is_some() {
            self.copy_selection_to_primary();
        }
    }

    /// 滚轮按行滚动。像素增量换算成行，避免触控板一次滚过十几行。
    fn scroll_wheel(&mut self, event: &ScrollEvent) {
        let lines = match event.delta {
            ScrollDelta::Lines(values) => values.y as i32,
            ScrollDelta::Pixels(values) => (values.y.as_f32() / self.row_height()).round() as i32,
        };
        if lines == 0 {
            return;
        }
        self.session_mut().scroll(lines * 3);
        let colors = self.ansi();
        self.session_mut().refresh(&colors);
    }

    /// 把窗口坐标换算成网格行列。越界值夹到网格边界，拖选不会越出网格。
    fn cell_at(&self, position: gpui::Point<gpui::Pixels>) -> (isize, usize) {
        let origin = self
            .grid_origin
            .lock()
            .ok()
            .and_then(|origin| *origin)
            .unwrap_or_else(|| gpui::point(px(0.), px(0.)));
        let relative = position - origin;
        let column = (relative.x.as_f32() / self.metrics.cell_width)
            .floor()
            .max(0.) as usize;
        let row = (relative.y.as_f32() / self.metrics.row_height)
            .floor()
            .max(0.) as isize;
        (row, column.min(self.session().columns.saturating_sub(1)))
    }

    /// 拖选结束后把选区同步到 PRIMARY，中键粘贴就能用。
    fn copy_selection_to_primary(&mut self) {
        let Some(text) = self.session().selection_text() else {
            return;
        };
        if !text.is_empty() {
            self.primary_queue.push(text);
        }
    }
}
