// @Time    : 2026/11/5 17:42
// @Author  : fzf
// @FileName: close_ops.rs
// @Software: Siskin

use super::SiskinApp;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CloseRequest {
    Session(u64),
    Window,
}

impl SiskinApp {
    pub(crate) fn close_session(&mut self, index: usize) {
        let Some(session) = self.sessions.get(index) else {
            return;
        };
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

    fn needs_close_confirmation(&self, index: usize) -> bool {
        self.sessions.get(index).is_some_and(|session| {
            session.is_running()
                && ((self.exit_on_child_exit && session.id == 0)
                    || session.pty().is_some_and(|pty| pty.has_running_job()))
        })
    }

    pub(crate) fn confirm_close(&mut self) {
        match self.close_request.take() {
            Some(CloseRequest::Window) => self.request_quit = true,
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
        if self.sessions.len() == 1 {
            // 保留状态直到窗口销毁，渲染和退出前的快照仍可访问活动会话。
            self.request_quit = true;
            return;
        }
        self.sessions.close(index);
        self.search = None;
        self.drag = None;
        self.session_scroll.scroll_to_item(self.active_index());
        self.request_focus = true;
        self.mark_dirty();
    }
}
