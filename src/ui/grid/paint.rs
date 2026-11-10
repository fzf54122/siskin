// @Time    : 2026/11/9 15:40
// @Author  : fzf
// @FileName: paint.rs
// @Software: Siskin

//! 一行的绘制。`mod.rs` 只负责 element 生命周期，这里把「铺背景、画文本、
//! 画链接底线」三个绘制阶段分开，每个函数只处理一种绘制任务。

use gpui::{App, Bounds, Window, fill, point, px, size};

use super::{RowLayout, to_hsla};

/// 背景矩形。选区与搜索高亮也在 `backgrounds` 里，顺序即绘制顺序，
/// 所以高亮会盖在选区之上，符合「当前匹配最醒目」的预期。
pub(super) fn row_backgrounds(row: &RowLayout, window: &mut Window) {
    for (position, extent, color) in &row.backgrounds {
        window.paint_quad(fill(Bounds::new(*position, *extent), to_hsla(*color)));
    }
}

pub(super) fn row_runs(row: &RowLayout, row_height: f32, window: &mut Window, cx: &mut App) {
    for (position, line) in &row.runs {
        let _ = line.paint(
            *position,
            px(row_height),
            gpui::TextAlign::Left,
            None,
            window,
            cx,
        );
    }
}

/// OSC 8 链接底线。画在单元格底部而不是贴基线的位置，
/// 这样和程序自己用 SGR 4 画的下划线观感一致。
pub(super) fn row_underlines(row: &RowLayout, row_height: f32, color: u32, window: &mut Window) {
    for (position, width) in &row.underlines {
        let baseline = position.y + px(row_height) - px(2.);
        window.paint_quad(fill(
            Bounds::new(point(position.x, baseline), size(*width, px(1.))),
            to_hsla(color),
        ));
    }
}
