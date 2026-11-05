// @Time    : 2026/11/4 20:02
// @Author  : fzf
// @FileName: app.rs
// @Software: Siskin

//! Application state. Rendering lives in `views`; everything here can be
//! exercised without a window.

use crate::{
    line::{Line, Role, Span},
    shell::Shell,
    terminal::Terminal,
    theme,
};
use gpui::{FocusHandle, ScrollHandle, SharedString};
use pulse_ui::gpui::{Context, point, px};

pub const ROW_HEIGHT: f32 = 20.;
pub const FONT_SIZE: f32 = 13.;
/// Advance width of Meslo/DejaVu-style monospace at `FONT_SIZE`, used to count
/// columns. A couple of pixels of slack keep lines from touching the edge.
pub const CHAR_WIDTH: f32 = 7.8;

pub struct Session {
    pub title: SharedString,
    pub role: &'static str,
    pub shell: Shell,
    pub terminal: Terminal,
    pub scroll: ScrollHandle,
}

impl Session {
    pub fn new(title: impl Into<SharedString>, role: &'static str) -> Self {
        Self {
            title: title.into(),
            role,
            shell: Shell::default(),
            terminal: Terminal::default(),
            scroll: ScrollHandle::new(),
        }
    }

    pub fn with_welcome(title: impl Into<SharedString>, role: &'static str) -> Self {
        let mut session = Self::new(title, role);
        session.welcome();
        session
    }

    /// Builds a session whose scrollback already shows `commands`, the way a
    /// tab looks when you come back to it. Commands run through the same shell
    /// as typed input, so the transcript can never drift from real behaviour.
    pub fn with_transcript(
        title: impl Into<SharedString>,
        role: &'static str,
        commands: &[&str],
    ) -> Self {
        let mut session = Self::new(title, role);
        session.welcome();
        for command in commands {
            let prompt = session.prompt_span();
            session.terminal.push_line(Line::new(vec![
                prompt,
                Span::new(" ", Role::Prompt),
                Span::new(*command, Role::Command),
            ]));
            let reply = session.shell.run(command);
            if reply.clear {
                session.terminal.clear();
            }
            for line in reply.lines {
                session.terminal.push_line(line);
            }
            if !session.terminal.lines().last().is_some_and(Line::is_blank) {
                session.terminal.push_blank();
            }
        }
        session
    }

    pub fn reset(&mut self) {
        self.shell = Shell::default();
        self.terminal.clear();
        self.terminal.clear_input();
        self.welcome();
        self.scroll_to_top();
    }

    pub fn scroll_to_top(&self) {
        self.scroll.set_offset(point(px(0.), px(0.)));
    }

    pub fn scroll_to_bottom(&self) {
        self.scroll.scroll_to_bottom();
    }

    pub fn welcome(&mut self) {
        let rows = vec![
            Line::new(vec![
                Span::new("Siskin ", Role::Command),
                Span::new("0.1.0", Role::Faint),
                Span::new(" · simulated terminal", Role::System),
            ]),
            Line::text(
                "真实布局与输入焦点，命令和文件树都是本地固定数据。",
                Role::Output,
            ),
            Line::default(),
            Line::new(vec![
                Span::new("输入 ", Role::Output),
                Span::new("help", Role::Path),
                Span::new(" 查看命令，或按 ", Role::Output),
                Span::new("Ctrl K", Role::Path),
                Span::new(" 打开命令面板。", Role::Output),
            ]),
            Line::default(),
        ];
        for row in rows {
            self.terminal.push_line(row);
        }
    }

    pub fn prompt_span(&self) -> Span {
        Span::new(self.shell.cwd_label(), Role::Prompt)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaletteAction {
    Clear,
    Reset,
    NewSession,
    NextSession,
    PreviousSession,
    Help,
    About,
    ToggleRail,
}

pub struct PaletteEntry {
    pub label: &'static str,
    /// English keywords so both "清空" and "clear" find the same entry.
    pub keywords: &'static str,
    pub hint: &'static str,
    pub keys: &'static str,
    pub action: PaletteAction,
}

const ENTRIES: [PaletteEntry; 8] = [
    PaletteEntry {
        label: "清空当前会话",
        keywords: "clear reset scrollback",
        hint: "只清掉滚动区，不重置目录",
        keys: "clear",
        action: PaletteAction::Clear,
    },
    PaletteEntry {
        label: "重置当前会话",
        keywords: "reset restore welcome",
        hint: "历史、目录和欢迎语一起恢复",
        keys: "",
        action: PaletteAction::Reset,
    },
    PaletteEntry {
        label: "新建会话",
        keywords: "new session tab open",
        hint: "独立的历史与目录",
        keys: "",
        action: PaletteAction::NewSession,
    },
    PaletteEntry {
        label: "下一会话",
        keywords: "next session switch tab",
        hint: "在左侧会话之间切换",
        keys: "Ctrl Tab",
        action: PaletteAction::NextSession,
    },
    PaletteEntry {
        label: "上一会话",
        keywords: "previous session switch tab",
        hint: "在左侧会话之间切换",
        keys: "Ctrl Shift Tab",
        action: PaletteAction::PreviousSession,
    },
    PaletteEntry {
        label: "显示命令帮助",
        keywords: "help commands manual",
        hint: "等同于输入 help",
        keys: "help",
        action: PaletteAction::Help,
    },
    PaletteEntry {
        label: "关于图标",
        keywords: "about icon logo",
        hint: "> _ * 的来历",
        keys: "siskin icon",
        action: PaletteAction::About,
    },
    PaletteEntry {
        label: "切换右侧信息栏",
        keywords: "toggle rail inspector panel",
        hint: "腾出更多终端宽度",
        keys: "",
        action: PaletteAction::ToggleRail,
    },
];

#[derive(Clone, Debug)]
pub struct Palette {
    pub query: String,
    pub selected: usize,
}

impl Palette {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            selected: 0,
        }
    }
}

impl Default for Palette {
    fn default() -> Self {
        Self::new()
    }
}

pub struct SiskinApp {
    pub sessions: Vec<Session>,
    pub active: usize,
    pub focus: FocusHandle,
    pub mono: SharedString,
    pub palette: Option<Palette>,
    pub rail: bool,
}

impl SiskinApp {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let mono = theme::mono_family(&cx.text_system().all_font_names());
        let mut sessions = vec![
            Session::with_transcript(
                "zsh",
                "main",
                &["siskin", "ls -l", "git status", "cat README.md"],
            ),
            Session::with_transcript("cargo", "watch", &["cargo check", "cargo test", "git log"]),
            Session::with_transcript("logs", "tail", &["ls", "pwd"]),
        ];
        for session in &mut sessions {
            session.terminal.scroll_to_bottom();
            session.scroll_to_bottom();
        }
        Self {
            sessions,
            active: 0,
            focus: cx.focus_handle(),
            mono: mono.into(),
            palette: None,
            rail: true,
        }
    }

    pub fn session(&self) -> &Session {
        &self.sessions[self.active.min(self.sessions.len() - 1)]
    }

    pub fn session_mut(&mut self) -> &mut Session {
        let index = self.active.min(self.sessions.len() - 1);
        &mut self.sessions[index]
    }

    pub fn select_session(&mut self, index: usize) {
        if index < self.sessions.len() {
            self.active = index;
        }
    }

    pub fn new_session(&mut self) {
        let index = self.sessions.len() + 1;
        self.sessions
            .push(Session::with_welcome(format!("siskin — {index}"), "new"));
        self.active = self.sessions.len() - 1;
    }

    pub fn cycle_session(&mut self, forward: bool) {
        let len = self.sessions.len();
        if len == 0 {
            return;
        }
        self.active = if forward {
            (self.active + 1) % len
        } else {
            (self.active + len - 1) % len
        };
    }

    pub fn submit(&mut self) {
        let index = self.active.min(self.sessions.len() - 1);
        let session = &mut self.sessions[index];
        let command = session.terminal.take_input();
        let prompt = session.prompt_span();
        let trimmed = command.trim().to_string();

        session.terminal.push_line(Line::new(vec![
            prompt,
            Span::new(" ", Role::Prompt),
            Span::new(trimmed.clone(), Role::Command),
        ]));

        let reply = session.shell.run(&trimmed);
        if reply.clear {
            session.terminal.clear();
        }
        for line in reply.lines {
            session.terminal.push_line(line);
        }
        // A command result should always end on a blank row before the next
        // prompt; otherwise wrapped output and the caret look glued together.
        if !session.terminal.lines().last().is_some_and(Line::is_blank) {
            session.terminal.push_blank();
        }
        session.terminal.scroll_to_bottom();
        session.scroll_to_bottom();
    }

    pub fn type_char(&mut self, ch: char) {
        self.session_mut().terminal.type_char(ch);
    }

    pub fn backspace(&mut self) {
        let session = self.session_mut();
        session.terminal.backspace();
    }

    pub fn clear_line(&mut self) {
        self.session_mut().terminal.clear_input();
    }

    pub fn history_back(&mut self) {
        let session = self.session_mut();
        if let Some(entry) = session.shell.history_back() {
            session.terminal.set_input(entry);
            session.terminal.scroll_to_bottom();
        }
    }

    pub fn history_forward(&mut self) {
        let session = self.session_mut();
        if let Some(entry) = session.shell.history_forward() {
            session.terminal.set_input(entry);
            session.terminal.scroll_to_bottom();
        }
    }

    pub fn complete(&mut self) {
        let session = self.session_mut();
        let current = session.terminal.input().to_string();
        if let Some(completed) = session.shell.complete(&current) {
            session.terminal.set_input(completed);
        }
    }

    pub fn scroll_page(&mut self, up: bool) {
        let session = self.session_mut();
        if up {
            session.terminal.scroll_up(12);
        } else {
            session.terminal.scroll_down(12);
        }
        let offset = -(session.terminal.scroll() as f32 * ROW_HEIGHT);
        session.scroll.set_offset(point(px(0.), px(offset)));
    }

    pub fn clear(&mut self) {
        let session = self.session_mut();
        session.terminal.clear();
        session.scroll_to_top();
    }

    pub fn open_palette(&mut self) {
        self.palette = Some(Palette::new());
    }

    pub fn close_palette(&mut self) {
        self.palette = None;
    }

    pub fn palette_entries(&self) -> Vec<&'static PaletteEntry> {
        let query = self
            .palette
            .as_ref()
            .map(|palette| palette.query.to_lowercase())
            .unwrap_or_default();
        ENTRIES
            .iter()
            .filter(|entry| {
                query.is_empty()
                    || entry.label.to_lowercase().contains(&query)
                    || entry.keywords.contains(&query)
                    || entry.keys.to_lowercase().contains(&query)
                    || entry.hint.to_lowercase().contains(&query)
            })
            .collect()
    }

    pub fn palette_type(&mut self, ch: char) {
        if let Some(palette) = self.palette.as_mut()
            && !ch.is_control()
        {
            palette.query.push(ch);
            palette.selected = 0;
        }
    }

    pub fn palette_backspace(&mut self) {
        if let Some(palette) = self.palette.as_mut() {
            palette.query.pop();
            palette.selected = 0;
        }
    }

    pub fn palette_move(&mut self, down: bool) {
        let count = self.palette_entries().len();
        if count == 0 {
            return;
        }
        if let Some(palette) = self.palette.as_mut() {
            palette.selected = if down {
                (palette.selected + 1).min(count - 1)
            } else {
                palette.selected.saturating_sub(1)
            };
        }
    }

    pub fn palette_confirm(&mut self) {
        let Some(palette) = self.palette.clone() else {
            return;
        };
        let entries = self.palette_entries();
        let Some(entry) = entries.get(palette.selected.min(entries.len().saturating_sub(1))) else {
            self.close_palette();
            return;
        };
        let action = entry.action;
        self.close_palette();
        self.run_palette_action(action);
    }

    pub fn run_palette_action(&mut self, action: PaletteAction) {
        match action {
            PaletteAction::Clear => self.clear(),
            PaletteAction::Reset => {
                self.session_mut().reset();
            }
            PaletteAction::NewSession => self.new_session(),
            PaletteAction::NextSession => self.cycle_session(true),
            PaletteAction::PreviousSession => self.cycle_session(false),
            PaletteAction::Help => {
                self.session_mut().terminal.set_input("help");
                self.submit();
            }
            PaletteAction::About => {
                self.session_mut().terminal.set_input("siskin icon");
                self.submit();
            }
            PaletteAction::ToggleRail => self.rail = !self.rail,
        }
    }

    /// Applies a key press to the app. Returns false when the key is unhandled,
    /// which lets the framework keep its normal behaviour.
    pub fn handle_key(
        &mut self,
        key: &str,
        key_char: Option<&str>,
        control: bool,
        platform: bool,
        alt: bool,
        shift: bool,
    ) -> bool {
        if let Some(palette) = self.palette.as_ref() {
            match key {
                "escape" => {
                    self.close_palette();
                    return true;
                }
                "enter" => {
                    self.palette_confirm();
                    return true;
                }
                "up" => {
                    self.palette_move(false);
                    return true;
                }
                "down" => {
                    self.palette_move(true);
                    return true;
                }
                "backspace" => {
                    self.palette_backspace();
                    return true;
                }
                _ => {}
            }
            if !control
                && !platform
                && !alt
                && !shift
                && let Some(ch) = key_char.and_then(single_char)
            {
                self.palette_type(ch);
                return true;
            }
            if !palette.query.is_empty() && (control || platform || alt) {
                return false;
            }
            return false;
        }

        if (control || platform) && key.eq_ignore_ascii_case("k") {
            self.open_palette();
            return true;
        }
        if (control || platform) && key.eq_ignore_ascii_case("l") {
            self.clear();
            return true;
        }
        if control && key == "tab" {
            self.cycle_session(!shift);
            return true;
        }

        match key {
            "enter" => self.submit(),
            "backspace" => self.backspace(),
            "tab" => self.complete(),
            "up" => self.history_back(),
            "down" => self.history_forward(),
            "pageup" => self.scroll_page(true),
            "pagedown" => self.scroll_page(false),
            "escape" => self.clear_line(),
            _ => {
                if control || platform || alt {
                    return false;
                }
                let Some(text) = key_char else {
                    return false;
                };
                let mut typed = false;
                for ch in text.chars() {
                    if !ch.is_control() {
                        self.type_char(ch);
                        typed = true;
                    }
                }
                return typed;
            }
        }
        true
    }

    pub fn run_help(&mut self) {
        self.run_palette_action(PaletteAction::Help);
    }

    pub fn command_count(&self) -> usize {
        self.session().shell.history().len()
    }

    pub fn line_count(&self) -> usize {
        self.session().terminal.lines().len()
    }

    pub fn scrollback_ratio(&self) -> f32 {
        (self.line_count() as f32 / crate::terminal::SCROLLBACK_LIMIT as f32).clamp(0., 1.)
    }

    #[cfg(test)]
    pub fn palette_open(&self) -> bool {
        self.palette.is_some()
    }
}

fn single_char(text: &str) -> Option<char> {
    let mut chars = text.chars();
    let first = chars.next()?;
    if chars.next().is_some() {
        None
    } else {
        Some(first)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{AppContext, TestAppContext};

    fn app(cx: &mut TestAppContext) -> gpui::Entity<SiskinApp> {
        cx.new(SiskinApp::new)
    }

    #[gpui::test]
    fn submitting_a_command_appends_prompt_and_output(cx: &mut TestAppContext) {
        let app = app(cx);
        app.update(cx, |app, _| {
            let before = app.command_count();
            for ch in "whoami".chars() {
                app.type_char(ch);
            }
            app.submit();
            let text = app
                .session()
                .terminal
                .lines()
                .iter()
                .map(Line::plain)
                .collect::<Vec<_>>()
                .join("\n");
            assert!(text.contains("whoami"));
            assert!(text.contains("fzf"));
            assert_eq!(app.command_count(), before + 1);
            assert!(app.session().terminal.input().is_empty());
        });
    }

    #[gpui::test]
    fn sessions_keep_independent_history_and_directories(cx: &mut TestAppContext) {
        let app = app(cx);
        app.update(cx, |app, _| {
            let before = app.command_count();
            app.type_char('c');
            app.type_char('d');
            app.type_char(' ');
            app.type_char('s');
            app.type_char('r');
            app.type_char('c');
            app.handle_key("enter", Some("\n"), false, false, false, false);
            assert_eq!(app.session().shell.cwd_label(), "~/src");

            app.new_session();
            assert_eq!(app.session().shell.cwd_label(), "~");
            assert_eq!(app.command_count(), 0);

            app.select_session(0);
            assert_eq!(app.session().shell.cwd_label(), "~/src");
            assert_eq!(app.command_count(), before + 1);
        });
    }

    #[gpui::test]
    fn keyboard_history_and_completion_drive_the_input_line(cx: &mut TestAppContext) {
        let app = app(cx);
        app.update(cx, |app, _| {
            let last = app.session().shell.history().last().cloned().unwrap();
            assert!(app.handle_key("up", None, false, false, false, false));
            assert_eq!(app.session().terminal.input(), last);
            app.handle_key("tab", None, false, false, false, false);
            assert_eq!(app.session().terminal.input(), format!("{last} "));
            app.handle_key("down", None, false, false, false, false);
            assert_eq!(app.session().terminal.input(), "");

            app.type_char('c');
            app.type_char('d');
            app.type_char(' ');
            app.type_char('s');
            app.handle_key("tab", None, false, false, false, false);
            assert_eq!(app.session().terminal.input(), "cd src/");
        });
    }

    #[gpui::test]
    fn palette_filters_and_runs_actions(cx: &mut TestAppContext) {
        let app = app(cx);
        app.update(cx, |app, _| {
            app.open_palette();
            for ch in "reset".chars() {
                app.palette_type(ch);
            }
            let labels: Vec<_> = app.palette_entries().iter().map(|e| e.label).collect();
            assert!(labels.contains(&"重置当前会话"));
            app.palette_confirm();
            assert!(app.palette.is_none());
            assert!(app.session().terminal.input().is_empty());
        });
    }

    #[gpui::test]
    fn palette_escape_and_empty_query_behave(cx: &mut TestAppContext) {
        let app = app(cx);
        app.update(cx, |app, _| {
            app.handle_key("k", None, true, false, false, false);
            assert!(app.palette_open());
            app.handle_key("escape", None, false, false, false, false);
            assert!(!app.palette_open());

            app.open_palette();
            app.palette_type('x');
            app.palette_type('y');
            app.palette_type('z');
            assert!(app.palette_entries().is_empty());
            app.palette_confirm();
            assert!(!app.palette_open());
        });
    }

    #[gpui::test]
    fn ctrl_l_clears_without_touching_the_shell_history(cx: &mut TestAppContext) {
        let app = app(cx);
        app.update(cx, |app, _| {
            let before = app.command_count();
            app.session_mut().shell.run("ls");
            app.handle_key("l", None, true, false, false, false);
            assert_eq!(app.line_count(), 0);
            assert_eq!(app.command_count(), before + 1);
        });
    }
}
