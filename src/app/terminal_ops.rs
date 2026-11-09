// @Time    : 2026/11/5 17:08
// @Author  : fzf
// @FileName: terminal_ops.rs
// @Software: Siskin

//! 终端操作：剪贴板、滚动、选区。写入 PTY 的入口只有 `write` / `paste`，
//! 这样「事件处理里不做 IO」的约束只在一处需要维护。

use super::SiskinApp;

impl SiskinApp {
    /// 清屏。按下与 `clear` 命令一致的序列：清可视区、清滚动区、光标回左上。
    /// 不走 `clear` 命令是因为用户可能把它别名掉了。
    pub(crate) fn clear_screen(&mut self) {
        self.write_bytes(b"\x1b[H\x1b[2J\x1b[3J");
        self.scroll_to_bottom();
    }

    pub(crate) fn write_bytes(&mut self, bytes: &[u8]) {
        self.session_mut().write(bytes);
    }

    /// 选区文本，不改应用状态；复制动作由 UI 层落进系统剪贴板。
    pub(crate) fn copy_selection_text(&self) -> Option<String> {
        self.session()
            .selection_text()
            .filter(|text| !text.is_empty())
    }

    /// `Ctrl+Shift+V` 与中键粘贴都走这里，换行统一交给 `io::keys::wrap_paste`。
    pub(crate) fn paste_text(&mut self, text: &str) {
        self.session_mut().paste(text);
    }

    /// 取走待写入的剪贴板内容。OSC 52 只允许写，不允许读。
    pub(crate) fn take_clipboard_writes(&mut self) -> Vec<String> {
        std::mem::take(&mut self.clipboard_queue)
    }

    pub(crate) fn take_primary_writes(&mut self) -> Vec<String> {
        std::mem::take(&mut self.primary_queue)
    }

    pub(crate) fn scroll_page(&mut self, up: bool) {
        let rows = self.session().rows as i32;
        let delta = if up { rows } else { -rows };
        self.session_mut().scroll(delta);
    }

    pub(crate) fn scroll_to_top(&mut self) {
        self.session_mut().scroll(i32::from(i16::MAX));
    }

    pub(crate) fn scroll_to_bottom(&mut self) {
        self.session_mut().scroll(-i32::from(i16::MAX));
    }

    /// 打开搜索条。已有查询词时保留，便于连续查找。
    pub(crate) fn open_search(&mut self) {
        let mut state = crate::app::SearchState::default();
        state.refresh(&self.session().frame);
        self.search = Some(state);
    }

    pub(crate) fn close_search(&mut self) {
        self.search = None;
    }

    pub(crate) fn search_type(&mut self, ch: char) {
        if self.search.is_none() {
            return;
        }
        let frame = self.session().frame.clone();
        if let Some(state) = self.search.as_mut() {
            state.type_char(ch);
            state.refresh(&frame);
        }
    }

    pub(crate) fn search_backspace(&mut self) {
        if self.search.is_none() {
            return;
        }
        let frame = self.session().frame.clone();
        if let Some(state) = self.search.as_mut() {
            state.backspace();
            state.refresh(&frame);
        }
    }

    pub(crate) fn step_match(&mut self, forward: bool) {
        let Some(state) = self.search.as_mut() else {
            return;
        };
        state.step(forward);
    }

    pub(crate) fn resize_grid(&mut self, columns: usize, rows: usize) {
        self.session_mut().resize(columns, rows);
    }

    /// 把鼠标事件转成 SGR/X10 序列。`Shift` 按下时始终留给本地选区，
    /// `Ctrl+Click` 打开光标下这一行的链接。返回是否真的打开了东西。
    /// 协议白名单与路径解析都在 `io::links`，这里只负责取当前行文本。
    pub(crate) fn open_link_at(&mut self, row: isize, column: usize) -> bool {
        let cwd = self.session().pty().and_then(|pty| pty.working_directory());
        let Some(line) = self.session().frame.rows.get(row.max(0) as usize) else {
            return false;
        };
        let text: String = line.cells.iter().map(|cell| cell.text.as_str()).collect();
        let found = crate::io::links::find(&text, cwd.as_deref());
        let Some((range, target)) = found.into_iter().find(|(range, _)| range.contains(&column))
        else {
            return false;
        };
        let _ = range;
        match crate::io::links::open(&target) {
            Ok(()) => true,
            Err(message) => {
                self.status = Some(message);
                false
            }
        }
    }

    /// 这是用户从鼠标上报的应用里复制文本的通行做法。
    pub(crate) fn forward_mouse(
        &mut self,
        row: isize,
        column: usize,
        button: u8,
        pressed: bool,
        shift: bool,
    ) -> bool {
        let Some(pty) = self.session().pty() else {
            return false;
        };
        let modes = crate::io::vt::key_modes(pty.mode());
        if !modes.mouse_reporting() || shift {
            return false;
        }
        let bytes = crate::io::keys::mouse_report(
            &modes,
            button,
            column,
            row.max(0) as usize,
            pressed,
            shift,
            false,
            false,
        );
        match bytes {
            Some(bytes) => {
                self.write_bytes(&bytes);
                true
            }
            None => false,
        }
    }
}
