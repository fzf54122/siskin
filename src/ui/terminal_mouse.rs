// @Time    : 2026/11/11
// @Author  : fzf
// @FileName: terminal_mouse.rs
// @Software: Siskin

//! 终端卡片的鼠标处理：按下取焦、拖选、滚轮、坐标换算。
//! 从 `terminal.rs` 拆出，只为守住单文件 400 行门禁；
//! 所有入口都按会话 id 参数化，分屏后每叶各算各的。

use gpui::{MouseDownEvent, ScrollDelta, ScrollEvent};

use super::*;

impl SiskinApp {
    /// 左键按下：先把焦点切到本叶，再按 `Ctrl+Click` 打开链接 →
    /// 鼠标上报 → 本地拖选的顺序走。焦点切换后 `session()` 就是本叶，
    /// 后面的链接与选区都不用再传 id。
    pub(super) fn press_left(&mut self, session_id: u64, event: &MouseDownEvent) {
        self.focus_pane(session_id);
        let (row, column) = self.cell_at(session_id, event.position);
        if event.modifiers.secondary() && self.open_link_at(row, column) {
            return;
        }
        if self.forward_mouse(session_id, row, column, 0, true, event.modifiers.shift) {
            return;
        }
        self.drag = Some((row, column));
        self.session_mut().clear_selection();
        self.session_mut().select(row, column);
    }

    /// 拖选端点是否移动到了新格子。同一格内抖动不改选区、不通知。
    /// 纯逻辑：鼠标回调本身在无显示器的 VM 上跑不了，判定由测试覆盖。
    fn drag_cell_changed(last: Option<(isize, usize)>, row: isize, column: usize) -> bool {
        last != Some((row, column))
    }

    /// 拖动中延长选区；没有按下时把移动事件转成鼠标上报。
    /// 返回是否发生了需要重绘的变化：拖选端点换格、或鼠标报告实际发出。
    /// 无变化的纯移动返回 `false`，调用方不 `notify`。
    /// 鼠标报告的发送路径不受抑制影响：`forward_mouse` 的返回值原样返回。
    /// 分隔线拖动中不处理选区；选区拖到别的叶上时也不跨叶改选区，
    /// 否则松开后 PRIMARY 里会是另一 pane 的文本。
    pub(super) fn drag_to(&mut self, session_id: u64, position: gpui::Point<gpui::Pixels>) -> bool {
        if self.pane_drag.is_some() {
            return false;
        }
        if self.drag.is_some() && session_id != self.active_pane {
            return false;
        }
        let (row, column) = self.cell_at(session_id, position);
        if self.drag.is_some() {
            if !Self::drag_cell_changed(self.drag, row, column) {
                return false;
            }
            self.drag = Some((row, column));
            if let Some(session) = self.session_by_id_mut(session_id) {
                session.update_selection(row, column);
            }
            return true;
        }
        self.forward_mouse(session_id, row, column, 3, true, false)
    }

    pub(super) fn release_left(&mut self, session_id: u64) {
        if self.drag.take().is_some() {
            self.copy_selection_to_primary(session_id);
        }
    }

    /// 滚轮按像素滚动。位移攒进会话的亚行余量，凑满一行才真正
    /// 滚一格，不足的部分交给渲染层平移，所以看起来是连续的。
    /// 滚的是指针所在的叶，不一定是焦点 pane。
    pub(super) fn scroll_wheel(&mut self, session_id: u64, event: &ScrollEvent) {
        let pixels = match event.delta {
            ScrollDelta::Lines(values) => values.y * self.row_height(),
            ScrollDelta::Pixels(values) => values.y.as_f32(),
        };
        if pixels == 0. {
            return;
        }
        let row_height = self.row_height();
        let changed = self
            .session_by_id_mut(session_id)
            .is_some_and(|session| session.scroll_pixels(pixels, row_height));
        if changed {
            let colors = self.ansi();
            if let Some(session) = self.session_by_id_mut(session_id) {
                session.refresh(&colors);
            }
        }
    }

    /// 把窗口坐标换算成网格行列。越界值夹到网格边界，拖选不会越出网格。
    /// 基准是该叶在 `resize_panes` 里记下的矩形（卡片原点 + 字体留白），
    /// 不读共享的 `grid_origin`：多叶时共享槽只剩最后一叶的值。
    pub(super) fn cell_at(
        &self,
        session_id: u64,
        position: gpui::Point<gpui::Pixels>,
    ) -> (isize, usize) {
        let origin = self
            .pane_bounds
            .get(&session_id)
            .map(|rect| {
                rect.origin + gpui::point(px(self.metrics.padding), px(self.metrics.padding))
            })
            .or_else(|| {
                self.grid_origin
                    .lock()
                    .ok()
                    .and_then(|origin| *origin)
                    .map(|origin| {
                        origin + gpui::point(px(self.metrics.padding), px(self.metrics.padding))
                    })
            })
            .unwrap_or_else(|| gpui::point(px(0.), px(0.)));
        let relative = position - origin;
        let column = (relative.x.as_f32() / self.metrics.cell_width)
            .floor()
            .max(0.) as usize;
        let row = (relative.y.as_f32() / self.metrics.row_height)
            .floor()
            .max(0.) as isize;
        let columns = self
            .session_by_id(session_id)
            .map(|session| session.columns)
            .unwrap_or(usize::MAX);
        (row, column.min(columns.saturating_sub(1)))
    }

    /// 拖选结束后把选区同步到 PRIMARY，中键粘贴就能用。
    pub(super) fn copy_selection_to_primary(&mut self, session_id: u64) {
        let Some(text) = self
            .session_by_id(session_id)
            .and_then(|session| session.selection_text())
        else {
            return;
        };
        if !text.is_empty() {
            self.primary_queue.push(text);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SiskinApp;

    #[test]
    fn dragging_to_a_new_cell_changes_the_selection() {
        assert!(SiskinApp::drag_cell_changed(Some((2, 3)), 2, 4));
        assert!(SiskinApp::drag_cell_changed(Some((2, 3)), 3, 3));
        assert!(SiskinApp::drag_cell_changed(None, 0, 0));
    }

    #[test]
    fn jitter_inside_the_same_cell_does_not_change_the_selection() {
        // 同一格内抖动：端点没换格，不改选区、不通知、不重复构帧。
        assert!(!SiskinApp::drag_cell_changed(Some((2, 3)), 2, 3));
        assert!(!SiskinApp::drag_cell_changed(Some((0, 0)), 0, 0));
    }
}
