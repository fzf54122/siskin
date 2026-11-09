// @Time    : 2026/11/5 19:22
// @Author  : fzf
// @FileName: cursor.rs
// @Software: Siskin

//! 光标绘制。四种形状与终端惯例一致：块、竖线、下划线、空心块；
//! 失焦时块状光标改用前景色，避免整块高亮抢走注意力。

use gpui::{Bounds, Pixels, Window, fill, point, px, size};

use crate::domain::RenderMetrics;
use crate::io::vt::{CursorShape, CursorView};

use super::to_hsla;

pub(crate) fn paint_cursor(
    cursor: CursorView,
    metrics: RenderMetrics,
    origin: gpui::Point<Pixels>,
    color: u32,
    fallback: u32,
    focused: bool,
    window: &mut Window,
) {
    let position = origin
        + point(
            px(cursor.column as f32 * metrics.cell_width),
            px(cursor.row as f32 * metrics.row_height),
        );
    let cell = size(px(metrics.cell_width), px(metrics.row_height));
    match cursor.shape {
        CursorShape::Hidden => {}
        CursorShape::Underline => window.paint_quad(fill(
            Bounds::new(
                position + point(px(0.), cell.height - px(2.)),
                size(cell.width, px(2.)),
            ),
            to_hsla(color),
        )),
        CursorShape::Beam => window.paint_quad(fill(
            Bounds::new(position, size(px(2.), cell.height)),
            to_hsla(color),
        )),
        CursorShape::HollowBlock => {
            let thickness = px(1.);
            window.paint_quad(fill(
                Bounds::new(position, size(cell.width, thickness)),
                to_hsla(color),
            ));
            window.paint_quad(fill(
                Bounds::new(
                    position + point(px(0.), cell.height - thickness),
                    size(cell.width, thickness),
                ),
                to_hsla(color),
            ));
        }
        CursorShape::Block => {
            let background = if focused { color } else { fallback };
            window.paint_quad(fill(Bounds::new(position, cell), to_hsla(background)));
        }
    }
}
