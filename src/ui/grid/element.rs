// @Time    : 2026/11/9 16:20
// @Author  : fzf
// @FileName: element.rs
// @Software: Siskin

//! GPUI 元素生命周期。网格自身只在 `mod.rs` 里构造与构帧，
//! 这里只实现 `Element`：布局、prepaint、paint。

use gpui::{
    App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, LayoutId, Pixels, Style,
    Window, fill, point, px, relative,
};

use super::{RowLayout, TerminalGrid, cursor, paint};

/// 带 alpha 的底色：只用于终端底色矩形——整窗玻璃开着时底色
/// 半透明，桌面从背后透出来；其余元素（文字、选区、光标）仍
/// 按 `to_hsla` 不透明画，可读性不受影响。
fn to_hsla_alpha(color: u32, alpha: f32) -> gpui::Hsla {
    let alpha = (alpha.clamp(0., 1.) * 255.).round() as u32;
    gpui::rgba((color << 8) | alpha).into()
}

impl Element for TerminalGrid {
    type RequestLayoutState = ();
    type PrepaintState = Vec<RowLayout>;

    fn id(&self) -> Option<ElementId> {
        self.debug_id.map(|id| ElementId::from(id.to_string()))
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();
        (window.request_layout(style, None, cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        _cx: &mut App,
    ) -> Self::PrepaintState {
        // 亚行平移只作用于绘制偏移；存进 `origin_slot` 的必须是未平移的锚点，
        // 否则鼠标换算会把整帧位移算进行列，点击随滚动一起漂。
        let anchor = bounds.origin + point(px(self.metrics.padding), px(self.metrics.padding));
        let origin = anchor + point(px(0.), px(self.offset_y));
        let visible_rows = (((bounds.size.height.as_f32() - self.metrics.padding * 2.)
            / self.metrics.row_height)
            .floor() as usize)
            .max(1)
            + 1;
        // P0 采样：只在开启时计时。`Instant` 本身很便宜，锁与记录
        // 都只在采样开启时才走，避免给正式渲染加固定开销。
        let shape_started = std::time::Instant::now();
        let layouts: Vec<RowLayout> = (0..self.frame.rows.len().min(visible_rows))
            .map(|row| self.shape_row(row, origin, window))
            .collect();
        if crate::domain::perf::is_enabled() {
            let draw_tasks = layouts
                .iter()
                .map(|row| row.runs.len() + row.backgrounds.len() + row.underlines.len())
                .sum::<usize>() as u32;
            let (hits, misses) = super::cache::stats_snapshot();
            crate::domain::perf::note_render(shape_started.elapsed(), draw_tasks, hits, misses);
        }
        self.store_cursor(origin, bounds.origin);
        layouts
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        layouts: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        window.paint_quad(fill(bounds, to_hsla_alpha(self.palette_bg, self.bg_alpha)));
        for row in layouts.iter() {
            paint::row_backgrounds(row, window);
            paint::row_runs(row, self.metrics.row_height, window, cx);
            paint::row_underlines(row, self.metrics.row_height, self.link_color, window);
        }
        // 输入法通过这个 handler 拿到预编辑串与候选框锚点；必须在 paint 阶段注册。
        window.handle_input(
            &self.focus,
            gpui::ElementInputHandler::new(bounds, self.view.clone()),
            cx,
        );
        // 滚进历史时光标不在可视区内：`frame.cursor.visible` 为假，
        // 不画底行那个被 clamp 出来的幻影光标。回到底部即恢复。
        if self.frame.cursor.visible {
            cursor::paint_cursor(
                self.frame.cursor,
                self.metrics,
                bounds.origin + point(px(self.metrics.padding), px(self.metrics.padding)),
                self.cursor_color,
                self.palette_fg,
                self.cursor_focused,
                window,
            );
        }
    }
}
