// @Time    : 2026/11/5 17:42
// @Author  : fzf
// @FileName: input.rs
// @Software: Siskin

use super::*;
use gpui::{
    Bounds, ElementInputHandler, MouseDownEvent, Pixels, Point, ShapedLine, TextRun, canvas, fill,
    point, size,
};
use std::sync::Arc;

struct InputPaint {
    line: ShapedLine,
    origin: Point<Pixels>,
    selection: Bounds<Pixels>,
    cursor: Bounds<Pixels>,
    selected: bool,
}

impl SiskinApp {
    pub(super) fn edit_field(
        &self,
        id: &'static str,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let edit = self.editor().cloned().unwrap_or_default();
        let font = self.font();
        let focus = self.focus.clone();
        let view = cx.entity();
        let bounds_slot = Arc::clone(&self.edit_bounds);
        let cursor_slot = Arc::clone(&self.cursor_slot);
        let palette = *p;
        div()
            .id(id)
            .debug_selector(move || id.into())
            .h(px(31.))
            .flex_1()
            .min_w_0()
            .overflow_hidden()
            .cursor_text()
            .on_click(|_, _, cx| cx.stop_propagation())
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|app, event: &MouseDownEvent, window, cx| {
                    app.place_edit_cursor(event.position, event.modifiers.shift, window);
                    app.focus.focus(window, cx);
                    cx.notify();
                    cx.stop_propagation();
                }),
            )
            .on_mouse_move(
                cx.listener(|app, event: &gpui::MouseMoveEvent, window, cx| {
                    if event.pressed_button == Some(MouseButton::Left) {
                        app.place_edit_cursor(event.position, true, window);
                        cx.notify();
                    }
                }),
            )
            .child(
                canvas(
                    move |bounds, window, _| {
                        let paint = shape_input(&edit, &font, bounds, &palette, window);
                        if let Ok(mut slot) = bounds_slot.lock() {
                            *slot = Some(bounds);
                        }
                        if let Ok(mut slot) = cursor_slot.lock() {
                            *slot = Some(paint.cursor);
                        }
                        paint
                    },
                    move |bounds, paint, window, cx| {
                        window.handle_input(
                            &focus,
                            ElementInputHandler::new(bounds, view.clone()),
                            cx,
                        );
                        window.with_content_mask(Some(gpui::ContentMask { bounds }), |window| {
                            if paint.selected {
                                window.paint_quad(fill(paint.selection, rgb(palette.tint)));
                            }
                            if let Err(error) = paint.line.paint(
                                paint.origin,
                                px(20.),
                                gpui::TextAlign::Left,
                                None,
                                window,
                                cx,
                            ) {
                                eprintln!("siskin: 无法绘制输入框：{error}");
                            }
                            if focus.is_focused(window) && !paint.selected {
                                window.paint_quad(fill(paint.cursor, rgb(palette.accent)));
                            }
                        });
                    },
                )
                .size_full(),
            )
    }

    fn place_edit_cursor(&mut self, position: Point<Pixels>, extend: bool, window: &mut Window) {
        let bounds = self.edit_bounds.lock().ok().and_then(|slot| *slot);
        let Some(bounds) = bounds else { return };
        let font = self.font();
        let palette = self.theme();
        let Some(edit) = self.editor_mut() else {
            return;
        };
        let paint = shape_input(edit, &font, bounds, &palette, window);
        let index = paint.line.closest_index_for_x(position.x - paint.origin.x);
        edit.move_to(index, extend);
    }
}

fn shape_input(
    edit: &crate::domain::text_edit::TextEdit,
    font: &SharedString,
    bounds: Bounds<Pixels>,
    p: &Palette,
    window: &mut Window,
) -> InputPaint {
    let text: SharedString = edit.text.clone().into();
    let line = window.text_system().shape_line(
        text.clone(),
        px(13.),
        &[TextRun {
            len: text.len(),
            font: gpui::font(font.clone()),
            color: rgb(p.ink).into(),
            background_color: None,
            underline: None,
            strikethrough: None,
        }],
        None,
    );
    let selection = edit.selection();
    let cursor = line.x_for_index(edit.cursor);
    let scroll = (cursor - bounds.size.width + px(4.)).max(px(0.));
    let origin = bounds.origin + point(-scroll, (bounds.size.height - px(20.)) / 2.);
    InputPaint {
        selection: Bounds::new(
            origin + point(line.x_for_index(selection.start), px(0.)),
            size(
                line.x_for_index(selection.end) - line.x_for_index(selection.start),
                px(20.),
            ),
        ),
        cursor: Bounds::new(origin + point(cursor, px(2.)), size(px(1.), px(16.))),
        line,
        origin,
        selected: !selection.is_empty(),
    }
}
