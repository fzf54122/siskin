// @Time    : 2026/11/5 17:42
// @Author  : fzf
// @FileName: close_ops.rs
// @Software: Siskin

use super::SiskinApp;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CloseRequest {
    Session(u64),
    /// 只关一个分屏叶，会话有运行任务时才弹确认。
    Pane(u64),
    Window,
}

impl SiskinApp {
    pub(crate) fn close_session(&mut self, index: usize) {
        self.commit_tab_rename();
        let Some(session) = self.sessions.get(index) else {
            return;
        };
        // 分屏叶不是标签：标签栏已不再列出它，但旧的调用点仍可能按
        // 下标关到它，转成分屏关闭，不让它按标签整条被删。
        if !self.tab_indices().contains(&index) {
            let id = session.id;
            self.close_pane(id);
            return;
        }
        if self.needs_close_confirmation(index) {
            self.close_request = Some(CloseRequest::Session(session.id));
            self.request_focus = true;
        } else {
            self.finish_close(index);
        }
    }

    pub(crate) fn request_exit(&mut self) {
        if self
            .sessions
            .iter()
            .any(|(index, _)| self.needs_close_confirmation(index))
        {
            self.close_request = Some(CloseRequest::Window);
            self.request_focus = true;
        } else {
            self.request_quit = true;
        }
    }

    pub(crate) fn needs_close_confirmation(&self, index: usize) -> bool {
        self.sessions.get(index).is_some_and(|session| {
            session.is_running()
                && ((self.exit_on_child_exit && session.id == 0)
                    || session.pty().is_some_and(|pty| pty.has_running_job()))
        })
    }

    pub(crate) fn confirm_close(&mut self) {
        match self.close_request.take() {
            Some(CloseRequest::Window) => self.request_quit = true,
            Some(CloseRequest::Pane(id)) => self.finish_close_pane(id),
            Some(CloseRequest::Session(id)) => {
                if let Some(index) = self.sessions.position(id) {
                    self.finish_close(index);
                }
            }
            None => {}
        }
    }

    pub(crate) fn cancel_close(&mut self) {
        self.close_request = None;
        self.request_focus = true;
    }

    fn finish_close(&mut self, index: usize) {
        // 最后一个「标签」才请求退出：`sessions.len()` 数的是会话，
        // 多叶标签本身就有多个会话，按它判会把整仓清成空。
        if self.tab_indices().len() <= 1 {
            // 保留状态直到窗口销毁，渲染和退出前的快照仍可访问活动会话。
            self.request_quit = true;
            return;
        }
        let Some(tab) = self.sessions.get(index) else {
            return;
        };
        let owner_id = tab.id;
        let leaf_ids = tab.panes.leaf_ids();
        // 先删其它叶（下标从大到小，删了不影响没删的下标），再删所有者。
        let mut others: Vec<usize> = leaf_ids
            .iter()
            .filter(|id| **id != owner_id)
            .filter_map(|id| self.sessions.position(*id))
            .collect();
        others.sort_unstable_by(|a, b| b.cmp(a));
        for leaf_index in others {
            self.sessions.close(leaf_index);
        }
        if let Some(owner_index) = self.sessions.position(owner_id) {
            self.sessions.close(owner_index);
        }
        self.pane_focus.remove(&owner_id);
        self.search = None;
        self.drag = None;
        self.pane_drag = None;
        self.session_scroll.scroll_to_item(self.active_index());
        self.restore_pane_focus();
        self.request_focus = true;
        self.mark_dirty();
    }
}
