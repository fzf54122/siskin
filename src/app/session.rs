// @Time    : 2026/11/5 16:40
// @Author  : fzf
// @FileName: session.rs
// @Software: Siskin

//! 会话仓储。每个会话持有一个真实 PTY，`SiskinApp` 只调这里的方法，
//! 不直接索引 `Vec`，窗口关闭由应用层处理。

use gpui::SharedString;

use crate::domain::SessionSpec;
use crate::domain::session::SessionState;
use crate::domain::shell_spec::ShellKind;
use crate::io::ansi::AnsiColors;
use crate::io::pty::Pty;
use crate::io::vt::{Frame, ScreenSize, VtEvent};

/// 新会话先按 80×24 派生，首帧拿到真实尺寸后再 resize 一次。
pub(crate) const INITIAL_COLUMNS: usize = 80;
pub(crate) const INITIAL_ROWS: usize = 24;
pub(crate) const SCROLLBACK_LINES: usize = 10_000;

pub(crate) struct Session {
    pub id: u64,
    pub title: SharedString,
    pub kind: ShellKind,
    pub state: SessionState,
    pub directory: Option<std::path::PathBuf>,
    pub window_title: Option<String>,
    pty: Option<Pty>,
    /// IME 预编辑串。只有提交时才写入 PTY。
    pub marked: Option<String>,
    pub frame: Frame,
    pub columns: usize,
    pub rows: usize,
}

impl Session {
    pub(crate) fn starting(id: u64, title: impl Into<SharedString>, kind: ShellKind) -> Self {
        Self {
            id,
            title: title.into(),
            kind,
            state: SessionState::Starting,
            directory: std::env::current_dir().ok(),
            window_title: None,
            pty: None,
            marked: None,
            frame: Frame::empty(INITIAL_ROWS),
            columns: INITIAL_COLUMNS,
            rows: INITIAL_ROWS,
        }
    }

    pub(crate) fn spawn_spec(
        &mut self,
        spec: &SessionSpec,
        events: async_channel::Sender<(u64, VtEvent)>,
        window_id: u64,
    ) -> Result<(), String> {
        let size = ScreenSize::new(self.columns, self.rows, SCROLLBACK_LINES);
        let id = self.id;
        match Pty::spawn(
            &spec.shell,
            spec.working_directory.clone(),
            size,
            events,
            id,
            window_id,
            &[],
        ) {
            Ok(pty) => {
                self.pty = Some(pty);
                self.state = SessionState::Running;
                Ok(())
            }
            Err(error) => {
                self.pty = None;
                let message = error.to_string();
                self.state = SessionState::Failed(message.clone());
                Err(message)
            }
        }
    }

    pub(crate) fn is_running(&self) -> bool {
        self.state.is_running() && self.pty.is_some()
    }

    pub(crate) fn display_title(&self) -> String {
        // 会话名优先跟随工作目录，`vim`/`ssh` 改 OSC 标题时标签保持稳定；
        // 目录名与显式会话名相同（如 `siskin`）时也一样。
        if let Some(name) = self.directory.as_ref().and_then(|path| path.file_name()) {
            let name = name.to_string_lossy();
            if self.title.as_ref() == name.as_ref()
                || self.title.as_ref() == "终端"
                || self.title.starts_with("会话 ")
            {
                return name.into_owned();
            }
        }
        self.title.to_string()
    }

    pub(crate) fn pty(&self) -> Option<&Pty> {
        self.pty.as_ref()
    }

    pub(crate) fn pty_mut(&mut self) -> Option<&mut Pty> {
        self.pty.as_mut()
    }

    pub(crate) fn write(&mut self, bytes: &[u8]) {
        if let Some(pty) = self.pty.as_mut() {
            pty.write(bytes);
        }
    }

    pub(crate) fn paste(&mut self, text: &str) {
        if let Some(pty) = self.pty.as_mut() {
            pty.paste(text);
        }
    }

    pub(crate) fn refresh(&mut self, colors: &AnsiColors) {
        if let Some(pty) = self.pty.as_ref() {
            self.frame = pty.frame(colors);
        }
    }

    pub(crate) fn resize(&mut self, columns: usize, rows: usize) {
        if columns == self.columns && rows == self.rows {
            return;
        }
        self.columns = columns.max(2);
        self.rows = rows.max(1);
        if let Some(pty) = self.pty.as_mut() {
            pty.resize(ScreenSize::new(self.columns, self.rows, SCROLLBACK_LINES));
        }
    }

    pub(crate) fn mark_exited(&mut self, code: Option<i32>) {
        self.state = SessionState::Exited(crate::domain::ExitInfo::from_status(code));
        if let Some(pty) = self.pty.as_mut() {
            pty.mark_exited();
        }
    }

    pub(crate) fn scroll(&mut self, delta: i32) {
        if let Some(pty) = self.pty.as_mut() {
            pty.scroll(delta);
        }
    }

    pub(crate) fn selection_text(&self) -> Option<String> {
        self.pty.as_ref().and_then(Pty::selection_text)
    }

    pub(crate) fn clear_selection(&mut self) {
        if let Some(pty) = self.pty.as_mut() {
            pty.clear_selection();
        }
    }
}

/// 会话集合。保证至少有一个会话，active 始终指向有效下标。
pub(crate) struct Sessions {
    items: Vec<Session>,
    active: usize,
    next_id: u64,
}

impl Sessions {
    pub(crate) fn new(first: Session) -> Self {
        let next_id = first.id + 1;
        Self {
            items: vec![first],
            active: 0,
            next_id,
        }
    }

    pub(crate) fn next_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub(crate) fn len(&self) -> usize {
        self.items.len()
    }

    pub(crate) fn active_index(&self) -> usize {
        self.active
    }

    pub(crate) fn active(&self) -> &Session {
        &self.items[self.active]
    }

    pub(crate) fn active_mut(&mut self) -> &mut Session {
        &mut self.items[self.active]
    }

    pub(crate) fn get(&self, index: usize) -> Option<&Session> {
        self.items.get(index)
    }

    pub(crate) fn get_mut(&mut self, index: usize) -> Option<&mut Session> {
        self.items.get_mut(index)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (usize, &Session)> {
        self.items.iter().enumerate()
    }

    pub(crate) fn position(&self, id: u64) -> Option<usize> {
        self.items.iter().position(|session| session.id == id)
    }

    pub(crate) fn select(&mut self, index: usize) -> bool {
        if index >= self.items.len() {
            return false;
        }
        self.active = index;
        true
    }

    pub(crate) fn add(&mut self, session: Session) -> usize {
        self.items.push(session);
        self.active = self.items.len() - 1;
        self.active
    }

    /// 最后一个会话由应用关闭窗口处理，不在仓储里重新派生进程。
    pub(crate) fn close(&mut self, index: usize) -> bool {
        if index >= self.items.len() {
            return false;
        }
        if self.items.len() == 1 {
            return false;
        }
        self.items.remove(index);
        if index < self.active {
            self.active -= 1;
        }
        self.active = self.active.min(self.items.len() - 1);
        true
    }

    pub(crate) fn cycle(&mut self, forward: bool) {
        let len = self.items.len();
        if len == 0 {
            return;
        }
        self.active = if forward {
            (self.active + 1) % len
        } else {
            (self.active + len - 1) % len
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sessions(count: usize) -> Sessions {
        let mut set = Sessions::new(Session::starting(0, "第一个", ShellKind::Zsh));
        for index in 1..count {
            let id = set.next_id();
            set.add(Session::starting(
                id,
                format!("会话 {index}"),
                ShellKind::Zsh,
            ));
        }
        set
    }

    #[test]
    fn closing_a_session_keeps_the_active_index_valid() {
        let mut set = sessions(3);
        set.select(1);
        set.close(0);
        assert_eq!(set.len(), 2);
        assert_eq!(set.active_index(), 0);
    }

    #[test]
    fn the_last_session_is_left_for_the_window_to_close() {
        let mut set = sessions(1);
        assert!(!set.close(0));
        assert_eq!(set.len(), 1);
        assert_eq!(set.active_index(), 0);
    }

    #[test]
    fn cycling_wraps_in_both_directions() {
        let mut set = sessions(3);
        set.select(0);
        set.cycle(false);
        assert_eq!(set.active_index(), 2);
        set.cycle(true);
        assert_eq!(set.active_index(), 0);
        assert!(!set.select(9));
    }

    #[test]
    fn session_ids_stay_unique_across_add_and_close() {
        let mut set = sessions(2);
        let id = set.next_id();
        set.add(Session::starting(id, "新会话", ShellKind::Bash));
        set.close(0);
        let ids: Vec<u64> = set.iter().map(|(_, session)| session.id).collect();
        assert_eq!(ids.len(), 2);
        assert_ne!(ids[0], ids[1]);
    }
}
