// @Time    : 2026/11/5 17:42
// @Author  : fzf
// @FileName: workspace_ops.rs
// @Software: Siskin

use std::path::PathBuf;
use std::time::{Duration, Instant};

use super::{Session, SiskinApp};
use crate::settings::{Page, SettingsSection};

impl SiskinApp {
    pub(crate) fn select_settings_section(&mut self, section: SettingsSection) {
        self.settings_section = section;
        self.color_edit = None;
        self.font_picker = None;
        self.settings_scroll
            .set_offset(gpui::point(gpui::px(0.), gpui::px(0.)));
    }

    pub(crate) fn set_page(&mut self, page: Page) {
        self.commit_tab_rename();
        self.page = page;
        self.palette = None;
        self.color_edit = None;
        self.font_picker = None;
        self.search = None;
        self.request_focus = true;
    }

    pub(crate) fn toggle_inspector(&mut self) {
        self.inspector_open = !self.inspector_open;
        self.request_focus = true;
    }

    pub(crate) fn select_session(&mut self, index: usize) {
        self.commit_tab_rename();
        if self.sessions.select(index) {
            self.restore_pane_focus();
            self.search = None;
            self.drag = None;
            self.pane_drag = None;
            self.session_scroll.scroll_to_item(index);
            self.refresh_directory(true);
            self.request_focus = true;
        }
    }

    pub(crate) fn cycle_session(&mut self, forward: bool) {
        self.sessions.cycle(forward);
        self.select_session(self.active_index());
        self.set_page(Page::Terminal);
    }

    pub(crate) fn new_session(&mut self) {
        self.commit_tab_rename();
        let directory = self
            .session()
            .pty()
            .and_then(|pty| pty.working_directory())
            .or_else(|| self.session().directory.clone());
        let id = self.sessions.next_id();
        let mut session = Session::starting(id, "终端", self.settings.shell);
        session.directory = directory;
        let index = self.sessions.add(session);
        self.active_pane = id;
        self.pane_focus.insert(id, id);
        let _ = self.spawn_session(index);
        self.session_scroll.scroll_to_item(index);
        self.mark_dirty();
        self.set_page(Page::Terminal);
    }

    pub(crate) fn refresh_directory(&mut self, force: bool) {
        if !force && self.directory_refreshed.elapsed() < Duration::from_millis(500) {
            return;
        }
        self.directory_refreshed = Instant::now();
        if let Some(directory) = self.session().pty().and_then(|pty| pty.working_directory()) {
            self.session_mut().directory = Some(directory);
        }
    }

    pub(crate) fn directory_label(&self) -> String {
        let Some(directory) = self.session().directory.as_ref() else {
            return "终端".into();
        };
        if let Some(home) = std::env::var_os("HOME").map(PathBuf::from)
            && let Ok(relative) = directory.strip_prefix(home)
        {
            return if relative.as_os_str().is_empty() {
                "~".into()
            } else {
                format!("~/{}", relative.display())
            };
        }
        directory.display().to_string()
    }
}
