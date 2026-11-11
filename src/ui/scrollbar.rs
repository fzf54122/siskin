// @Time    : 2026/11/11
// @Author  : Lux
// @FileName: scrollbar.rs
// @Software: Siskin

//! 网格右侧的细滚动条：位置与长度反映 `display_offset / history_size`。
//! 只有 `history > 0` 时才画；无历史就不画，不占地方、不 crash。
//!
//! 换算全是纯函数（可单测）：thumb 在轨道里的占比只由
//! 可视行数、历史行数与当前偏移决定，与渲染无关。
//! 点击轨道、拖动 thumb 都在 `terminal_page` 的鼠标回调里走
//! `scrollbar_drag` 状态（与分隔线拖动同一套模式），
//! 拖动中指针离开细条也能继续收到移动事件。

use gpui::{Context, MouseButton, MouseDownEvent, Stateful, relative};

use super::*;

/// thumb 最短 24px：历史再长也不能缩成一条看不见的线。
const MIN_THUMB_HEIGHT: f32 = 24.;

/// 轨道按像素算的几何。`display_offset == 0` 是底部，
/// `display_offset == history` 是顶部。
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ScrollbarGeometry {
    pub track_height: f32,
    pub viewport_rows: usize,
    pub history: usize,
    pub thumb_top: f32,
    pub thumb_height: f32,
}

impl ScrollbarGeometry {
    /// 点击轨道上 `y`（相对轨道顶）时，thumb 应居中到该处对应的偏移。
    /// 上端点是顶部，下端点是底部，越界夹到 `[0, history]`。
    pub(crate) fn offset_at_y(&self, y: f32) -> usize {
        let usable = self.track_height - self.thumb_height;
        if usable <= 0. || self.history == 0 {
            return 0;
        }
        let fraction = ((y - self.thumb_height / 2.) / usable).clamp(0., 1.);
        ((self.history as f32 * (1. - fraction)).round() as usize).min(self.history)
    }

    /// 从 `start_offset` 开始拖动 thumb，指针纵向移动 `dy` 像素
    /// （向下为正）后的新偏移。向下拉是回到底部，偏移减小。
    pub(crate) fn offset_for_drag(&self, start_offset: usize, dy: f32) -> usize {
        let usable = self.track_height - self.thumb_height;
        if usable <= 0. || self.history == 0 {
            return start_offset.min(self.history);
        }
        let moved = dy / usable * self.history as f32;
        (start_offset as f32 - moved)
            .round()
            .clamp(0., self.history as f32) as usize
    }
}

/// 轨道高 `track_height` 像素时的几何；无历史时返回 `None`（不画）。
pub(crate) fn geometry(
    track_height: f32,
    viewport_rows: usize,
    history: usize,
    display_offset: usize,
) -> Option<ScrollbarGeometry> {
    if track_height <= 0. || viewport_rows == 0 || history == 0 {
        return None;
    }
    let total = viewport_rows + history;
    let thumb_height = (track_height * viewport_rows as f32 / total as f32)
        .max(MIN_THUMB_HEIGHT)
        .min(track_height);
    let offset = display_offset.min(history);
    let thumb_top = (track_height - thumb_height) * (history - offset) as f32 / history as f32;
    Some(ScrollbarGeometry {
        track_height,
        viewport_rows,
        history,
        thumb_top,
        thumb_height,
    })
}

/// 拖动中的 thumb。指针位置与起始偏移一起算目标偏移，
/// 轨道高度与历史行数在按下时快照，拖动中不重算。
#[derive(Clone, Copy, Debug)]
pub(crate) struct ScrollbarDrag {
    pub session_id: u64,
    pub start_y: f32,
    pub start_offset: usize,
    pub geometry: ScrollbarGeometry,
}

impl ScrollbarDrag {
    pub(crate) fn offset_at(&self, y: f32) -> usize {
        self.geometry
            .offset_for_drag(self.start_offset, y - self.start_y)
    }
}

impl SiskinApp {
    /// 该叶滚动条的当前几何与轨道顶在窗口里的 y 坐标。
    /// 轨道就是卡片本身：`pane_bounds` 是卡片矩形，与 `cell_at` 同一套基准。
    fn scrollbar_geometry(&self, session_id: u64) -> Option<(ScrollbarGeometry, f32)> {
        let rect = self.pane_bounds.get(&session_id)?;
        let session = self.session_by_id(session_id)?;
        let history = session.history_size()?;
        let viewport_rows = session.frame.rows.len();
        let geometry = geometry(
            rect.size.height.as_f32(),
            viewport_rows,
            history,
            session.frame.display_offset,
        )?;
        Some((geometry, rect.origin.y.as_f32()))
    }

    /// 窗口坐标 `y` 在该叶轨道上对应的偏移；没有滚动条时为 `None`。
    fn scrollbar_offset_at(&self, session_id: u64, y: f32) -> Option<usize> {
        let (geometry, top) = self.scrollbar_geometry(session_id)?;
        Some(geometry.offset_at_y(y - top))
    }

    /// 开始拖 thumb：快照轨道几何与当前偏移，后续移动只算增量。
    pub(super) fn start_scrollbar_drag(&mut self, session_id: u64, y: f32) {
        let Some((geometry, _)) = self.scrollbar_geometry(session_id) else {
            return;
        };
        let Some(session) = self.session_by_id(session_id) else {
            return;
        };
        self.scrollbar_drag = Some(ScrollbarDrag {
            session_id,
            start_y: y,
            start_offset: session.frame.display_offset,
            geometry,
        });
    }

    /// 拖动中按指针位置更新偏移；偏移没变时返回 `false`，调用方不重绘。
    pub(super) fn update_scrollbar_drag(&mut self, y: f32) -> bool {
        let Some(drag) = self.scrollbar_drag else {
            return false;
        };
        let offset = drag.offset_at(y);
        let current = self
            .session_by_id(drag.session_id)
            .map(|session| session.frame.display_offset);
        if current == Some(offset) {
            return false;
        }
        self.scroll_to_offset(drag.session_id, offset);
        true
    }

    /// 松开即结束拖动；之前在拖时返回 `true`。
    pub(super) fn end_scrollbar_drag(&mut self) -> bool {
        self.scrollbar_drag.take().is_some()
    }

    /// 网格右侧的滚动条本体。无历史时返回 `None`，`grid_layer`
    /// 里用 `when_some` 挂，不占地方。thumb 用 `faint` 低对比色，
    /// 细条不抢终端内容的注意力。
    pub(super) fn scrollbar(
        &self,
        session_id: u64,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Option<Stateful<Div>> {
        let (geometry, _) = self.scrollbar_geometry(session_id)?;
        let top = geometry.thumb_top / geometry.track_height;
        let height = geometry.thumb_height / geometry.track_height;
        let thumb = div()
            .id(("scrollbar-thumb", session_id))
            .debug_selector(|| "scrollbar-thumb".into())
            .absolute()
            .top(relative(top))
            .h(relative(height))
            .right(px(2.))
            .w(px(6.))
            .rounded(px(3.))
            .bg(rgb(p.faint))
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |app, event: &MouseDownEvent, _, cx| {
                    app.focus_pane(session_id);
                    app.start_scrollbar_drag(session_id, event.position.y.as_f32());
                    cx.notify();
                    // 不冒泡到轨道：否则同一次按下既开始拖动又跳一次位置。
                    cx.stop_propagation();
                }),
            );
        Some(
            div()
                .id(("scrollbar", session_id))
                .debug_selector(|| "scrollbar".into())
                .absolute()
                .top(px(0.))
                .bottom(px(0.))
                .right(px(0.))
                .w(px(10.))
                .cursor_pointer()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |app, event: &MouseDownEvent, _, cx| {
                        app.focus_pane(session_id);
                        if let Some(offset) =
                            app.scrollbar_offset_at(session_id, event.position.y.as_f32())
                        {
                            app.scroll_to_offset(session_id, offset);
                        }
                        cx.notify();
                    }),
                )
                .child(thumb),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TRACK: f32 = 400.;

    #[test]
    fn no_history_means_no_scrollbar() {
        assert!(geometry(TRACK, 24, 0, 0).is_none());
        assert!(geometry(TRACK, 0, 10, 0).is_none());
        assert!(geometry(0., 24, 10, 0).is_none());
    }

    #[test]
    fn thumb_size_follows_viewport_share_and_bottom_is_zero_offset() {
        let geometry = geometry(TRACK, 24, 76, 0).unwrap();
        assert!((geometry.thumb_height - 96.).abs() < 0.01);
        assert!((geometry.thumb_top - 304.).abs() < 0.01);
    }

    #[test]
    fn top_offset_puts_thumb_at_the_top() {
        let geometry = geometry(TRACK, 24, 76, 76).unwrap();
        assert!(geometry.thumb_top.abs() < 0.01);
    }

    #[test]
    fn middle_offset_puts_thumb_in_the_middle() {
        let geometry = geometry(TRACK, 24, 76, 38).unwrap();
        assert!((geometry.thumb_top - 152.).abs() < 0.01);
    }

    #[test]
    fn thumb_never_shrinks_below_the_minimum() {
        let geometry = geometry(TRACK, 24, 10_000, 0).unwrap();
        assert!((geometry.thumb_height - MIN_THUMB_HEIGHT).abs() < 0.01);
        assert!((geometry.thumb_top - (TRACK - MIN_THUMB_HEIGHT)).abs() < 0.01);
    }

    #[test]
    fn clicking_the_track_centers_the_thumb_there() {
        let geometry = geometry(TRACK, 24, 76, 0).unwrap();
        assert_eq!(geometry.offset_at_y(0.), 76);
        assert_eq!(geometry.offset_at_y(TRACK), 0);
        assert_eq!(geometry.offset_at_y(200.), 38);
        assert_eq!(geometry.offset_at_y(-100.), 76);
        assert_eq!(geometry.offset_at_y(1000.), 0);
    }

    #[test]
    fn dragging_down_returns_toward_the_bottom() {
        let geometry = geometry(TRACK, 24, 76, 0).unwrap();
        assert_eq!(geometry.offset_for_drag(76, 0.), 76);
        assert_eq!(geometry.offset_for_drag(76, 304.), 0);
        assert_eq!(geometry.offset_for_drag(76, 152.), 38);
        assert_eq!(geometry.offset_for_drag(76, 9999.), 0);
        assert_eq!(geometry.offset_for_drag(0, -9999.), 76);
    }
}
