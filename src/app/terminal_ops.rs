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
        let colors = self.ansi();
        self.session_mut().scroll(delta);
        self.session_mut().refresh(&colors);
        self.sync_search_frame();
    }

    pub(crate) fn scroll_to_top(&mut self) {
        let colors = self.ansi();
        self.session_mut().scroll(i32::from(i16::MAX));
        // 跳到边界时清掉亚行余量，否则贴顶仍留着半行偏移。
        self.session_mut().scroll_offset.snap_to_top();
        self.session_mut().refresh(&colors);
        self.sync_search_frame();
    }

    pub(crate) fn scroll_to_bottom(&mut self) {
        let colors = self.ansi();
        self.session_mut().scroll(-i32::from(i16::MAX));
        self.session_mut().scroll_offset.snap_to_bottom();
        self.session_mut().refresh(&colors);
        self.sync_search_frame();
    }

    /// 命令间跳转：焦点会话按记录滚到目标命令起始行。
    /// 没有可跳的命令时不动，不报错、不刷帧。
    pub(crate) fn jump_command(&mut self, forward: bool) {
        if !self.session_mut().jump_command(forward) {
            return;
        }
        let colors = self.ansi();
        self.session_mut().refresh(&colors);
        self.sync_search_frame();
    }

    /// 滚动只换 `display_offset`，匹配的绝对行不变：把新帧指针同步进
    /// 搜索缓存，下一轮刷新不用为一次滚动白扫一遍历史。
    fn sync_search_frame(&mut self) {
        if self.search.is_none() {
            return;
        }
        let frame = self.session().frame_snapshot();
        if let Some(state) = self.search.as_mut() {
            state.sync_frame(frame);
        }
    }

    /// 打开搜索条。已有查询词时保留，便于连续查找。
    pub(crate) fn open_search(&mut self) {
        self.search = Some(crate::app::SearchState::default());
        self.refresh_search();
    }

    pub(crate) fn close_search(&mut self) {
        self.search = None;
    }

    /// 搜索条打开时重算历史匹配。帧指针、查询词与选项都没变时，
    /// `SearchState` 内部直接复用，不再锁 PTY 扫历史。
    pub(crate) fn refresh_search(&mut self) {
        if self.search.is_none() {
            return;
        }
        let frame = self.session().frame_snapshot();
        let Some(state) = self.search.as_ref() else {
            return;
        };
        // 空查询词也要走一遍 `refresh_history`，由它清空旧匹配；
        // 只在「非空且缓存命中」时跳过。
        if !state.query.is_empty() && !state.needs_rescan(&frame) {
            return;
        }
        let (query, case_sensitive, regex) =
            (state.query.clone(), state.case_sensitive, state.regex);
        let result = match self.session().pty() {
            Some(pty) => pty.search_history(&query, case_sensitive, regex),
            // 没有 PTY 也校验正则合法性：非法正则的错误要能立刻看见，
            // 而不是等到会话跑起来才出现。
            None => crate::io::history_search::validate_pattern(&query, case_sensitive, regex)
                .map(|()| Vec::new()),
        };
        if let Some(state) = self.search.as_mut() {
            state.refresh_history(frame, move || result);
        }
    }

    pub(crate) fn search_type(&mut self, ch: char) {
        if self.search.is_none() {
            return;
        }
        if let Some(state) = self.search.as_mut() {
            state.type_char(ch);
        }
        self.refresh_search();
    }

    pub(crate) fn search_backspace(&mut self) {
        if self.search.is_none() {
            return;
        }
        if let Some(state) = self.search.as_mut() {
            state.backspace();
        }
        self.refresh_search();
    }

    pub(crate) fn toggle_search_case(&mut self) {
        if let Some(state) = self.search.as_mut() {
            state.toggle_case();
        }
        self.refresh_search();
    }

    pub(crate) fn toggle_search_regex(&mut self) {
        if let Some(state) = self.search.as_mut() {
            state.toggle_regex();
        }
        self.refresh_search();
    }

    /// 步进到上 / 下一个匹配，并把该行滚进可见区再高亮。
    /// 匹配在历史深处时一样能滚到；两端循环环绕。
    pub(crate) fn step_match(&mut self, forward: bool) {
        let Some(state) = self.search.as_mut() else {
            return;
        };
        state.step(forward);
        let Some(span) = state.current_span() else {
            return;
        };
        let rows = self.session().rows as i32;
        let display_offset = self.session().frame.display_offset as i32;
        // 可见窗口的绝对行区间：[-display_offset, -display_offset + rows)。
        let top = -display_offset;
        let bottom = top + rows - 1;
        let delta = if span.line < top {
            -span.line - display_offset
        } else if span.line > bottom {
            rows - 1 - span.line - display_offset
        } else {
            0
        };
        if delta == 0 {
            return;
        }
        let colors = self.ansi();
        self.session_mut().scroll(delta);
        self.session_mut().refresh(&colors);
        self.sync_search_frame();
    }

    /// 把鼠标事件转成 SGR/X10 序列。`Shift` 按下时始终留给本地选区，
    /// `Ctrl+Click` 打开光标下这一行的链接。返回是否真的打开了东西。
    /// 优先级：OSC 8 超链接 → 裸路径解析（`io::links` 的白名单与
    /// `path:line:col` 解析都在那里，这里只负责取当前行）。
    pub(crate) fn open_link_at(&mut self, row: isize, column: usize) -> bool {
        // 行内容与 cwd 都在函数前段取完，后面写 `self.status` 不再与
        // frame 的不可变借用冲突。
        let (linked, cwd, text) = {
            let Some(line) = self.session().frame.rows.get(row.max(0) as usize) else {
                return false;
            };
            let cwd = self.session().pty().and_then(|pty| pty.working_directory());
            (
                line.cells
                    .get(column)
                    .and_then(|cell| cell.link.as_deref().map(str::to_string)),
                cwd,
                line.cells
                    .iter()
                    .map(|cell| cell.text.as_str())
                    .collect::<String>(),
            )
        };
        // OSC 8：程序自己圈出的链接，地址可信度高于从文本猜。
        if let Some(uri) = linked {
            return self.open_uri(&uri);
        }
        let found = crate::io::links::find(&text, cwd.as_deref());
        let Some((_, target)) = found.into_iter().find(|(range, _)| range.contains(&column)) else {
            return false;
        };
        match crate::io::links::open(&target) {
            Ok(()) => true,
            Err(message) => {
                self.status = Some(message);
                false
            }
        }
    }

    /// 只放开白名单协议。终端内容来自不可信程序，
    /// `javascript:` 这类协议不能直接交给系统 opener。
    fn open_uri(&mut self, uri: &str) -> bool {
        let lower = uri.trim().to_ascii_lowercase();
        if !(lower.starts_with("http://")
            || lower.starts_with("https://")
            || lower.starts_with("file://"))
        {
            self.status = Some(format!("已拦截不可信链接协议：{uri}"));
            return false;
        }
        match crate::io::links::open(&crate::io::links::Target::Web(uri.to_string())) {
            Ok(()) => true,
            Err(message) => {
                self.status = Some(message);
                false
            }
        }
    }

    /// 这是用户从鼠标上报的应用里复制文本的通行做法。
    /// 行列与写回都按调用方给的 pane 算，不走焦点会话，
    /// 否则点非焦点 pane 会把序列发进焦点 pane 的程序。
    pub(crate) fn forward_mouse(
        &mut self,
        session_id: u64,
        row: isize,
        column: usize,
        button: u8,
        pressed: bool,
        shift: bool,
    ) -> bool {
        let Some(pty) = self
            .session_by_id(session_id)
            .and_then(|session| session.pty())
        else {
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
                if let Some(session) = self.session_by_id_mut(session_id) {
                    session.write(&bytes);
                }
                true
            }
            None => false,
        }
    }
}
