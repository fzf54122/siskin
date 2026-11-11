// @Time    : 2026/11/11
// @Author  : fzf
// @FileName: pane_ops.rs
// @Software: Siskin

//! 分屏的应用层操作：分裂、焦点循环、关闭、按叶算尺寸、分隔线拖动。
//!
//! 约定：标签就是 `Sessions` 里的一条记录，分屏产生的会话只是
//! 该标签树里的叶，不进标签栏（`tab_indices` 按「不被别的树包含」
//! 判定）。`session()` / `session_mut()` 永远指向当前聚焦的叶，
//! 所以键盘输入、滚动、搜索天然落在焦点 pane 上，不用逐处改。

use gpui::{Bounds, Pixels, Point};

use super::SiskinApp;
use crate::app::pane::{PANE_DIVIDER, PaneDrag, PaneTree, SplitDirection};
use crate::app::{CloseRequest, Session};

impl SiskinApp {
    pub(crate) fn session_by_id(&self, id: u64) -> Option<&Session> {
        let index = self.sessions.position(id)?;
        self.sessions.get(index)
    }

    pub(crate) fn session_by_id_mut(&mut self, id: u64) -> Option<&mut Session> {
        let index = self.sessions.position(id)?;
        self.sessions.get_mut(index)
    }

    /// 标签下标：会话 id 不被任何别的会话的树包含的才是标签，
    /// 分屏叶只活在所属标签的树里，不在标签栏出现。
    pub(crate) fn tab_indices(&self) -> Vec<usize> {
        self.sessions
            .iter()
            .filter(|(index, session)| {
                !self
                    .sessions
                    .iter()
                    .any(|(other, tree)| other != *index && tree.panes.contains(session.id))
            })
            .map(|(index, _)| index)
            .collect()
    }

    pub(crate) fn tab_index_containing(&self, id: u64) -> Option<usize> {
        self.sessions
            .iter()
            .find_map(|(index, session)| session.panes.contains(id).then_some(index))
    }

    pub(crate) fn active_tab_tree(&self) -> PaneTree {
        self.sessions.active().panes.clone()
    }

    pub(crate) fn pane_leaf_count(&self) -> usize {
        self.active_tab_tree().leaf_ids().len()
    }

    /// 点击叶或按键切换都走这里：只改焦点叶，不动标签，
    /// 并记住该标签的焦点叶，切换标签回来时能回到它。
    pub(crate) fn focus_pane(&mut self, id: u64) {
        if id == self.active_pane || !self.active_tab_tree().contains(id) {
            return;
        }
        self.active_pane = id;
        self.drag = None;
        let tab_id = self.sessions.active().id;
        self.pane_focus.insert(tab_id, id);
        self.request_focus = true;
    }

    /// 切换标签后回到该标签上次聚焦的叶，记不住的取首叶。
    pub(crate) fn restore_pane_focus(&mut self) {
        let tab_id = self.sessions.active().id;
        let tree = self.active_tab_tree();
        let remembered = self
            .pane_focus
            .get(&tab_id)
            .copied()
            .filter(|id| tree.contains(*id));
        self.active_pane = remembered.or_else(|| tree.first_leaf()).unwrap_or(tab_id);
    }

    /// 在当前标签里分裂出一个新会话。按 tmux 惯例焦点跟到新 pane。
    /// 新会话先按 `Sessions::add` 进仓储（它会暂时把活动标签切过去），
    /// 派生完再切回原标签、把新叶挂进原标签的树。
    pub(crate) fn split_pane(&mut self, direction: SplitDirection) {
        if self.page != crate::settings::Page::Terminal {
            return;
        }
        let current = self.active_pane;
        let Some((new_id, _)) = self.create_pane_session() else {
            return;
        };
        let tab_id = self.sessions.active().id;
        if !self
            .sessions
            .active_mut()
            .panes
            .split(current, direction, new_id)
        {
            // 叶找不到了（不应发生）：撤回刚建的会话，不留孤儿。
            if let Some(index) = self.sessions.position(new_id) {
                self.sessions.close(index);
            }
            return;
        }
        self.active_pane = new_id;
        self.pane_focus.insert(tab_id, new_id);
        self.search = None;
        self.drag = None;
        self.request_focus = true;
        self.mark_dirty();
    }

    fn create_pane_session(&mut self) -> Option<(u64, usize)> {
        let tab_index = self.active_index();
        let directory = self.session().pty().and_then(|pty| pty.working_directory());
        let directory = directory.or_else(|| self.session().directory.clone());
        let new_id = self.sessions.next_id();
        let mut session = Session::starting(new_id, "终端", self.settings.shell);
        session.directory = directory;
        let new_index = self.sessions.add(session);
        // 派生失败也保留会话（状态是 Failed，界面看得见原因），
        // 不走回滚，否则用户连失败提示都看不到。
        let _ = self.spawn_session(new_index);
        self.sessions.select(tab_index);
        Some((new_id, new_index))
    }

    pub(crate) fn focus_pane_cycle(&mut self, forward: bool) {
        let tree = self.active_tab_tree();
        let next = if forward {
            tree.focus_next(self.active_pane)
        } else {
            tree.focus_prev(self.active_pane)
        };
        if let Some(id) = next {
            self.focus_pane(id);
        }
    }

    /// 关闭一个 pane。最后一个叶时按现有语义关闭整个标签；
    /// 有运行任务时走与会话关闭同一套确认框（`CloseRequest::Pane`）。
    pub(crate) fn close_pane(&mut self, id: u64) {
        let tree = self.active_tab_tree();
        if tree.leaf_ids().len() <= 1 {
            let index = self.active_index();
            self.close_session(index);
            return;
        }
        if !tree.contains(id) {
            return;
        }
        let Some(index) = self.sessions.position(id) else {
            return;
        };
        if self.needs_close_confirmation(index) {
            self.close_request = Some(CloseRequest::Pane(id));
            self.request_focus = true;
            return;
        }
        self.finish_close_pane(id);
    }

    /// 确认框确认后、或无需确认时真正关一个 pane。
    /// 叶是标签所有者时，树先过户给相邻叶，再关所有者会话，
    /// 这样标签不会因为所有者被关而丢掉整棵树。
    pub(crate) fn finish_close_pane(&mut self, id: u64) {
        let Some(owner_index) = self.tab_index_containing(id) else {
            return;
        };
        let Some(owner_id) = self.sessions.get(owner_index).map(|session| session.id) else {
            return;
        };
        let Some(mut tree) = self
            .sessions
            .get(owner_index)
            .map(|session| session.panes.clone())
        else {
            return;
        };
        if tree.leaf_ids().len() <= 1 || !tree.close_leaf(id) {
            return;
        }
        // `focus_next` 在删叶前取：删完再取就只剩相邻叶本身，
        // 循环会绕回被关的叶，焦点就错了。
        let Some(neighbor) = self
            .sessions
            .get(owner_index)
            .map(|session| session.panes.clone())
            .and_then(|before| before.focus_next(id))
        else {
            return;
        };
        if id == owner_id {
            if let Some(index) = self.sessions.position(neighbor)
                && let Some(session) = self.sessions.get_mut(index)
            {
                session.panes = tree;
            }
            if let Some(index) = self.sessions.position(id) {
                self.sessions.close(index);
            }
            if let Some(index) = self.sessions.position(neighbor) {
                self.sessions.select(index);
            }
            self.pane_focus.remove(&owner_id);
        } else {
            if let Some(session) = self.sessions.get_mut(owner_index) {
                session.panes = tree;
            }
            if let Some(index) = self.sessions.position(id) {
                self.sessions.close(index);
            }
            if let Some(index) = self.sessions.position(owner_id) {
                self.sessions.select(index);
            }
        }
        self.active_pane = neighbor;
        let tab_id = self.sessions.active().id;
        self.pane_focus.insert(tab_id, neighbor);
        self.after_pane_close();
    }

    fn after_pane_close(&mut self) {
        self.search = None;
        self.drag = None;
        self.pane_drag = None;
        self.session_scroll.scroll_to_item(self.active_index());
        self.request_focus = true;
        self.mark_dirty();
    }

    /// 按活动标签的树把一块终端区切给每个叶，记下每叶的窗口矩形，
    /// 再按各自区域算行列、调对应会话的尺寸。复用 `metrics` 的
    /// cell 宽高推导，不另起一套字体度量。行列不变时 `Session::resize`
    /// 是空操作，所以每帧调一次也无额外 PTY 通知。
    pub(crate) fn resize_panes(&mut self, area: Bounds<Pixels>) {
        self.pane_area = area;
        let rects = self.active_tab_tree().layout(area);
        self.pane_bounds = rects.iter().map(|(id, rect)| (*id, *rect)).collect();
        for (id, rect) in rects {
            let columns = self.metrics.columns(rect.size.width.as_f32());
            let rows = self.metrics.rows(rect.size.height.as_f32());
            if let Some(session) = self.session_by_id_mut(id) {
                session.resize(columns, rows);
            }
        }
    }

    /// 分隔线按下：按第二个子树的首叶找到那条线，记下方向、
    /// 起始比例与分隔线总长（两侧子树矩形的并集减线厚）。
    pub(crate) fn start_pane_drag(&mut self, second_leaf: u64, position: Point<Pixels>) {
        let tree = self.active_tab_tree();
        let Some((direction, ratio, _)) = tree.split_info(second_leaf) else {
            return;
        };
        let Some((first_ids, second_ids)) = tree.split_leaves(second_leaf) else {
            return;
        };
        let Some(total) = self.subtree_span(&first_ids, &second_ids, direction) else {
            return;
        };
        self.pane_drag = Some(PaneDrag {
            second_leaf,
            horizontal: direction == SplitDirection::Horizontal,
            start: if direction == SplitDirection::Horizontal {
                position.y.as_f32()
            } else {
                position.x.as_f32()
            },
            ratio,
            total,
        });
    }

    fn subtree_span(
        &self,
        first: &[u64],
        second: &[u64],
        direction: SplitDirection,
    ) -> Option<f32> {
        let span = |ids: &[u64]| -> Option<(f32, f32)> {
            let mut range: Option<(f32, f32)> = None;
            for id in ids {
                let rect = self.pane_bounds.get(id)?;
                let (start, end) = match direction {
                    SplitDirection::Vertical => (
                        rect.origin.x.as_f32(),
                        rect.origin.x.as_f32() + rect.size.width.as_f32(),
                    ),
                    SplitDirection::Horizontal => (
                        rect.origin.y.as_f32(),
                        rect.origin.y.as_f32() + rect.size.height.as_f32(),
                    ),
                };
                range = Some(match range {
                    Some((low, high)) => (low.min(start), high.max(end)),
                    None => (start, end),
                });
            }
            range
        };
        let (first_low, _) = span(first)?;
        let (_, second_high) = span(second)?;
        let total = second_high - first_low - PANE_DIVIDER;
        if total > 0. { Some(total) } else { None }
    }

    /// 分隔线拖动：位移 / 总长加到起始比例上，树里钳到 0.1..=0.9，
    /// 下一帧 `resize_panes` 按新比例重算各叶行列并通知 PTY。
    pub(crate) fn update_pane_drag(&mut self, position: Point<Pixels>) {
        let Some(drag) = self.pane_drag else {
            return;
        };
        let at = if drag.horizontal {
            position.y.as_f32()
        } else {
            position.x.as_f32()
        };
        let ratio = drag.ratio + (at - drag.start) / drag.total.max(1.);
        if self
            .sessions
            .active_mut()
            .panes
            .set_ratio(drag.second_leaf, ratio)
        {
            self.mark_dirty();
        }
    }

    pub(crate) fn end_pane_drag(&mut self) {
        self.pane_drag = None;
    }
}
