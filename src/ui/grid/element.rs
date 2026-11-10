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

use super::{RowLayout, TerminalGrid, cursor, paint, to_hsla};

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
        let layouts = (0..self.frame.rows.len().min(visible_rows))
            .map(|row| self.shape_row(row, origin, window))
            .collect();
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
        window.paint_quad(fill(bounds, to_hsla(self.palette_bg)));
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
