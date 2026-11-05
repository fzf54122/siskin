// @Time    : 2026/11/4 20:02
// @Author  : fzf
// @FileName: app.rs
// @Software: Siskin

//! Application state. Rendering lives in `views`; everything here can be
//! exercised without a window.

use crate::{
    line::{Line, Role, Span},
    settings::{Page, Settings, SettingsSection, ShellKind},
    shell::Shell,
    terminal::Terminal,
    theme,
    theme::Palette,
};
use gpui::{FocusHandle, ScrollHandle, SharedString};
use pulse_ui::gpui::{Context, point, px};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorTarget {
    Accent,
    Background,
    Foreground,
}

pub struct ColorEdit {
    pub target: ColorTarget,
    pub draft: String,
    pub error: Option<String>,
}

pub struct Session {
    pub title: SharedString,
    pub kind: ShellKind,
    pub shell: Shell,
    pub terminal: Terminal,
    pub scroll: ScrollHandle,
}

impl Session {
    pub fn new(title: impl Into<SharedString>, kind: ShellKind) -> Self {
        let title = title.into();
        Self {
            title,
            kind,
            shell: Shell::default(),
            terminal: Terminal::default(),
            scroll: ScrollHandle::new(),
        }
    }

    pub fn reset(&mut self) {
        self.shell = Shell::default();
        self.terminal.clear();
        self.terminal.clear_input();
        self.scroll_to_top();
    }

    pub fn scroll_to_top(&self) {
        self.scroll.set_offset(point(px(0.), px(0.)));
    }

    pub fn scroll_to_bottom(&self) {
        self.scroll.scroll_to_bottom();
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
    OpenSettings,
    OpenTerminal,
    NextTheme,
    NextShell,
    FontLarger,
    FontSmaller,
    ToggleInspector,
}

pub struct PaletteEntry {
    pub label: &'static str,
    /// English keywords so both "清空" and "clear" find the same entry.
    pub keywords: &'static str,
    pub hint: &'static str,
    pub keys: &'static str,
    pub action: PaletteAction,
}

const ENTRIES: [PaletteEntry; 14] = [
    PaletteEntry {
        label: "切换会话信息",
        keywords: "inspector panel session information",
        hint: "显示或收起右侧会话信息",
        keys: "Ctrl Shift I",
        action: PaletteAction::ToggleInspector,
    },
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
        label: "打开设置",
        keywords: "settings preferences theme font",
        hint: "主题、字体、shell",
        keys: "Ctrl ,",
        action: PaletteAction::OpenSettings,
    },
    PaletteEntry {
        label: "回到终端",
        keywords: "terminal back workspace",
        hint: "关闭设置页",
        keys: "Esc",
        action: PaletteAction::OpenTerminal,
    },
    PaletteEntry {
        label: "切换配色主题",
        keywords: "theme palette color dark light",
        hint: "跟随系统 / 浅色 / 深色 / 自定义",
        keys: "Ctrl Shift T",
        action: PaletteAction::NextTheme,
    },
    PaletteEntry {
        label: "切换 shell",
        keywords: "shell zsh bash fish sh",
        hint: "zsh / bash / fish / sh",
        keys: "Ctrl Shift S",
        action: PaletteAction::NextShell,
    },
    PaletteEntry {
        label: "放大字号",
        keywords: "font bigger larger zoom",
        hint: "终端等宽字号 +1 档",
        keys: "Ctrl +",
        action: PaletteAction::FontLarger,
    },
    PaletteEntry {
        label: "缩小字号",
        keywords: "font smaller zoom out",
        hint: "终端等宽字号 -1 档",
        keys: "Ctrl -",
        action: PaletteAction::FontSmaller,
    },
];

#[derive(Clone, Debug)]
pub struct CommandPalette {
    pub query: String,
    pub selected: usize,
}

impl CommandPalette {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            selected: 0,
        }
    }
}

impl Default for CommandPalette {
    fn default() -> Self {
        Self::new()
    }
}

pub struct SiskinApp {
    pub sessions: Vec<Session>,
    pub active: usize,
    pub focus: FocusHandle,
    pub mono: SharedString,
    pub ui: SharedString,
    pub mono_fonts: Vec<String>,
    pub palette: Option<CommandPalette>,
    pub settings_section: SettingsSection,
    pub inspector_open: bool,
    pub page: Page,
    pub settings: Settings,
    /// Updated by GPUI's desktop appearance observer.
    pub system_dark: bool,
    /// The exported global menu, when the desktop provides a registrar.
    pub menu: Option<crate::dbusmenu::GlobalMenu>,
    pub menu_state: crate::dbusmenu::MenuState,
    pub color_edit: Option<ColorEdit>,
    preferences_path: Option<std::path::PathBuf>,
}

impl SiskinApp {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let installed = cx.text_system().all_font_names();
        let mono_fonts = theme::mono_fonts(&installed);
        let mono = mono_fonts[0].clone();
        let ui = theme::ui_family(&installed).to_string();
        let system_dark = false;
        let mut settings = Settings::default();
        settings.resolve_theme(system_dark);
        let sessions = vec![Session::new("终端", settings.shell)];
        Self {
            sessions,
            active: 0,
            focus: cx.focus_handle(),
            mono: mono.into(),
            ui: ui.into(),
            mono_fonts,
            palette: None,
            settings_section: SettingsSection::Appearance,
            inspector_open: false,
            page: Page::Terminal,
            settings,
            system_dark,
            menu: None,
            menu_state: crate::dbusmenu::MenuState::Unavailable,
            color_edit: None,
            preferences_path: None,
        }
    }

    pub fn theme(&self) -> Palette {
        self.settings
            .palette_for(self.settings.mode, self.system_dark)
    }

    pub fn set_theme_mode(&mut self, mode: crate::settings::ThemeMode) {
        self.settings.set_mode(mode, self.system_dark);
        self.sync_menu();
        self.save_preferences();
    }

    pub fn set_system_dark(&mut self, dark: bool) {
        self.system_dark = dark;
        self.settings.resolve_theme(dark);
    }

    pub fn set_custom_accent(&mut self, accent: usize) {
        self.settings.custom.accent = accent;
        self.settings.custom.accent_rgb = None;
        if self.settings.mode == crate::settings::ThemeMode::Custom {
            self.settings.resolve_theme(self.system_dark);
        }
        self.save_preferences();
    }

    pub fn toggle_custom_dark(&mut self) {
        self.settings.custom.dark = !self.settings.custom.dark;
        if self.settings.mode == crate::settings::ThemeMode::Custom {
            self.settings.resolve_theme(self.system_dark);
        }
        self.save_preferences();
    }

    pub fn load_preferences(&mut self, path: std::path::PathBuf) {
        match crate::preferences::load(&path) {
            Ok(prefs) => {
                prefs.apply(&mut self.settings, &self.mono_fonts, self.system_dark);
                self.session_mut().kind = self.settings.shell;
            }
            Err(error) => eprintln!("siskin: 无法读取设置，保留原文件并使用默认值：{error}"),
        }
        self.preferences_path = Some(path);
    }

    pub fn save_preferences(&self) {
        if let Some(path) = &self.preferences_path {
            let prefs =
                crate::preferences::Preferences::from_settings(&self.settings, &self.mono_fonts);
            if let Err(error) = crate::preferences::save(path, &prefs) {
                eprintln!("siskin: 无法保存设置：{error}");
            }
        }
    }

    pub fn edit_color(&mut self, target: ColorTarget) {
        let color = match target {
            ColorTarget::Accent => self.settings.custom.accent_color(),
            ColorTarget::Background => self.settings.custom.background.map_or(
                if self.settings.custom.dark {
                    theme::GRAPHITE.terminal
                } else {
                    theme::PAPER.terminal
                },
                |color| color.0,
            ),
            ColorTarget::Foreground => self.settings.custom.foreground.map_or(
                if self.settings.custom.dark {
                    theme::GRAPHITE.output
                } else {
                    theme::PAPER.output
                },
                |color| color.0,
            ),
        };
        self.color_edit = Some(ColorEdit {
            target,
            draft: format!("{color:06x}"),
            error: None,
        });
    }

    pub fn commit_color(&mut self) {
        let Some(edit) = self.color_edit.as_mut() else {
            return;
        };
        let color = match theme::ThemeColor::try_from(edit.draft.clone()) {
            Ok(color) => color,
            Err(error) => {
                edit.error = Some(error);
                return;
            }
        };
        match edit.target {
            ColorTarget::Accent => self.settings.custom.accent_rgb = Some(color),
            ColorTarget::Background => self.settings.custom.background = Some(color),
            ColorTarget::Foreground => self.settings.custom.foreground = Some(color),
        }
        self.color_edit = None;
        self.set_theme_mode(crate::settings::ThemeMode::Custom);
    }

    pub fn font(&self) -> SharedString {
        self.mono_fonts
            .get(self.settings.font_family)
            .cloned()
            .unwrap_or_else(|| self.mono.to_string())
            .into()
    }

    pub fn font_size(&self) -> f32 {
        self.settings.font_size()
    }

    pub fn row_height(&self) -> f32 {
        self.settings.row_height()
    }

    pub fn char_width(&self) -> f32 {
        self.settings.char_width()
    }

    pub fn select_settings_section(&mut self, section: SettingsSection) {
        self.settings_section = section;
        self.color_edit = None;
    }

    pub fn set_page(&mut self, page: Page) {
        self.page = page;
        self.palette = None;
        self.color_edit = None;
    }

    /// Settings changes that affect prompt rendering start a fresh prompt in
    /// the active session so the shell switch is visible immediately.
    pub fn set_shell(&mut self, kind: ShellKind) {
        self.settings.shell = kind;
        let session = self.session_mut();
        session.kind = kind;
        session.terminal.push_line(Line::new(vec![
            Span::new(format!("{} ", kind.name()), crate::line::Role::System),
            Span::new(kind.startup(), crate::line::Role::Faint),
        ]));
        session.terminal.scroll_to_bottom();
        session.scroll_to_bottom();
        self.sync_menu();
        self.save_preferences();
    }

    pub fn cycle_theme(&mut self) {
        self.settings.next_theme(self.system_dark);
        self.sync_menu();
        self.save_preferences();
    }

    /// Rebuilds the menu exported to the desktop panel. Called at startup and
    /// whenever a menu-visible setting changes.
    pub fn sync_menu(&mut self) {
        let Some(menu) = self.menu.as_ref() else {
            return;
        };
        let layout =
            crate::dbusmenu::build_layout(self.settings.mode.name(), self.session().kind.name());
        menu.set_layout(layout);
    }

    /// Runs one action dispatched by the desktop menu.
    pub fn run_menu_action(&mut self, action: crate::dbusmenu::MenuAction) {
        use crate::dbusmenu::MenuAction;
        match action {
            MenuAction::NewSession => self.new_session(),
            MenuAction::CloseSession => self.close_session(self.active),
            MenuAction::Clear => self.clear(),
            MenuAction::Quit => {}
            MenuAction::CommandPalette => self.open_palette(),
            MenuAction::Settings => self.set_page(Page::Settings),
            MenuAction::CycleThemeMode => self.cycle_theme(),
            MenuAction::FontLarger => self.step_font(true),
            MenuAction::FontSmaller => self.step_font(false),
            MenuAction::ToggleInspector => self.inspector_open = !self.inspector_open,
            MenuAction::SetShell(kind) => self.set_shell(kind),
        }
        self.sync_menu();
    }

    pub fn install_menu(&mut self, window_id: u32, cx: &mut Context<Self>) {
        let layout =
            crate::dbusmenu::build_layout(self.settings.mode.name(), self.session().kind.name());
        match crate::dbusmenu::install(window_id, layout) {
            Ok(menu) => {
                let events = menu.events();
                self.menu = Some(menu);
                self.menu_state = crate::dbusmenu::MenuState::Starting;
                self.attach_menu_events(events, cx);
            }
            Err(error) => eprintln!("siskin: 无法启动全局菜单：{error}"),
        }
    }

    pub fn attach_menu_events(
        &mut self,
        events: async_channel::Receiver<crate::dbusmenu::MenuEvent>,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |this, cx| {
            while let Ok(event) = events.recv().await {
                if this
                    .update(cx, |this, cx| {
                        this.apply_menu_event(event, cx);
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
    }

    pub fn apply_menu_event(&mut self, event: crate::dbusmenu::MenuEvent, cx: &mut Context<Self>) {
        use crate::dbusmenu::{MenuAction, MenuEvent};
        match event {
            MenuEvent::Action(MenuAction::Quit) => cx.quit(),
            MenuEvent::Action(action) => self.run_menu_action(action),
            MenuEvent::State(state) => self.menu_state = state,
        }
        cx.notify();
    }

    pub fn step_font(&mut self, up: bool) {
        self.settings.step_font(up);
        self.save_preferences();
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
            self.sync_menu();
        }
    }

    pub fn close_session(&mut self, index: usize) {
        if index >= self.sessions.len() {
            return;
        }
        if self.sessions.len() == 1 {
            self.sessions[0].reset();
        } else {
            self.sessions.remove(index);
            if index < self.active {
                self.active -= 1;
            }
            self.active = self.active.min(self.sessions.len() - 1);
        }
        self.sync_menu();
    }

    pub fn new_session(&mut self) {
        let index = self.sessions.len() + 1;
        self.sessions
            .push(Session::new(format!("会话 {index}"), self.settings.shell));
        self.active = self.sessions.len() - 1;
        self.set_page(Page::Terminal);
        self.sync_menu();
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
        self.sync_menu();
    }

    pub fn submit(&mut self) {
        let index = self.active.min(self.sessions.len() - 1);
        let session = &mut self.sessions[index];
        let command = session.terminal.take_input();
        let trimmed = command.trim();
        // An empty Enter only moves the cursor to a fresh prompt, it does not
        // print a bare prompt row into the scrollback.
        if trimmed.is_empty() {
            session.terminal.scroll_to_bottom();
            session.scroll_to_bottom();
            return;
        }
        let prompt = session.prompt_span();
        let trimmed = trimmed.to_string();

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
        let row_height = self.row_height();
        let session = self.session_mut();
        if up {
            session.terminal.scroll_up(12);
        } else {
            session.terminal.scroll_down(12);
        }
        let offset = -(session.terminal.scroll() as f32 * row_height);
        session.scroll.set_offset(point(px(0.), px(offset)));
    }

    pub fn clear(&mut self) {
        let session = self.session_mut();
        session.terminal.clear();
        session.scroll_to_top();
    }

    pub fn open_palette(&mut self) {
        self.palette = Some(CommandPalette::new());
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
            PaletteAction::OpenSettings => self.set_page(Page::Settings),
            PaletteAction::OpenTerminal => self.set_page(Page::Terminal),
            PaletteAction::NextTheme => self.cycle_theme(),
            PaletteAction::NextShell => {
                self.settings.next_shell();
                self.set_shell(self.settings.shell);
            }
            PaletteAction::FontLarger => self.step_font(true),
            PaletteAction::FontSmaller => self.step_font(false),
            PaletteAction::ToggleInspector => self.inspector_open = !self.inspector_open,
        }
        self.sync_menu();
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
        if let Some(edit) = self.color_edit.as_mut() {
            match key {
                "enter" => self.commit_color(),
                "escape" => self.color_edit = None,
                "backspace" => {
                    edit.draft.pop();
                    edit.error = None;
                }
                "a" if control || platform => {
                    edit.draft.clear();
                    edit.error = None;
                }
                _ if !control && !platform && !alt => {
                    if let Some(text) = key_char {
                        for ch in text.chars().filter(char::is_ascii_hexdigit) {
                            if edit.draft.len() < 6 {
                                edit.draft.push(ch);
                            }
                        }
                        edit.error = None;
                    }
                }
                _ => {}
            }
            return true;
        }
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
        if (control || platform) && key == "," {
            self.set_page(Page::Settings);
            return true;
        }
        if (control || platform) && key == "1" {
            self.set_page(Page::Terminal);
            return true;
        }
        if (control || platform) && shift && key.eq_ignore_ascii_case("i") {
            self.inspector_open = !self.inspector_open;
            self.sync_menu();
            return true;
        }
        if (control || platform) && shift && key.eq_ignore_ascii_case("t") {
            self.cycle_theme();
            return true;
        }
        if (control || platform) && shift && key.eq_ignore_ascii_case("s") {
            self.settings.next_shell();
            self.set_shell(self.settings.shell);
            return true;
        }
        if (control || platform) && matches!(key, "=" | "+") {
            self.step_font(true);
            return true;
        }
        if (control || platform) && key == "-" {
            self.step_font(false);
            return true;
        }

        // The settings page owns its own keys; the terminal must not swallow
        // typing while the user is browsing preferences.
        if self.page == Page::Settings {
            return match key {
                "escape" | "enter" => {
                    self.set_page(Page::Terminal);
                    true
                }
                "up" => {
                    self.step_font(true);
                    true
                }
                "down" => {
                    self.step_font(false);
                    true
                }
                "left" | "right" | "tab" => {
                    self.cycle_theme();
                    true
                }
                _ => false,
            };
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

    pub fn command_count(&self) -> usize {
        self.session().shell.history().len()
    }

    pub fn line_count(&self) -> usize {
        self.session().terminal.lines().len()
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
            app.session_mut().shell.run("help");
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
