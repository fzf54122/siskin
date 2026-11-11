// @Time    : 2026/11/5 16:45
// @Author  : fzf
// @FileName: mod.rs
// @Software: Siskin

//! 应用状态。渲染在 `ui`，按键链路在 `app/keys.rs`，动作在
//! `app/actions.rs`，PTY 事件由 `app/events.rs` 消费。

mod actions;
mod clipboard;
mod close_ops;
mod editor_ops;
mod events;
mod keys;
mod menu_ops;
mod palette;
mod prefs_ops;
mod search_ops;
mod session;
mod terminal_ops;
mod theme_ops;
mod workspace_ops;

pub(crate) use actions::{PALETTE_ENTRIES, PaletteEntry};
pub(crate) use close_ops::CloseRequest;
pub(crate) use editor_ops::FontPicker;
pub(crate) use keys::Handled;
pub(crate) use palette::CommandPalette;
pub(crate) use search_ops::SearchState;
pub(crate) use session::{Session, Sessions};

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use gpui::{Bounds, FocusHandle, Pixels, SharedString};

use crate::domain::RenderMetrics;
use crate::domain::{Action, SessionSpec, ShellKind};
use crate::io::ansi::AnsiColors;
use crate::io::dbusmenu::{GlobalMenu, MenuState};
use crate::io::vt::VtEvent;
use crate::settings::{Page, Settings, SettingsSection};
use crate::theme;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ColorTarget {
    Accent,
    Background,
    Foreground,
}

pub(crate) struct ColorEdit {
    pub target: ColorTarget,
    pub draft: crate::domain::text_edit::TextEdit,
    pub error: Option<String>,
    pub base: theme::CustomTheme,
}

pub(crate) struct SiskinApp {
    pub sessions: Sessions,
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
    pub menu: Option<GlobalMenu>,
    pub menu_state: MenuState,
    pub color_edit: Option<ColorEdit>,
    pub(crate) font_picker: Option<FontPicker>,
    pub(crate) close_request: Option<CloseRequest>,
    pub(crate) session_scroll: gpui::ScrollHandle,
    pub(crate) settings_scroll: gpui::ScrollHandle,
    pub(crate) request_focus: bool,
    pub(crate) directory_refreshed: Instant,
    pub(crate) edit_bounds: Arc<std::sync::Mutex<Option<Bounds<Pixels>>>>,
    /// 最近一次配置读写失败的提示，设置页据此显示一行错误。
    pub save_error: Option<String>,
    /// 网格度量，渲染时由字体实际字形宽度推导。
    pub metrics: RenderMetrics,
    /// 已经写给窗口的标题，用于避免每帧重复设置。
    pub(crate) window_title: Option<String>,
    /// `-e <cmd>` 会话：子进程退出即关闭窗口。
    pub exit_on_child_exit: bool,
    /// 会话派生失败时顶栏显示的一行提示。
    pub status: Option<String>,
    pub window_id: u64,
    pub(crate) drag: Option<(f32, f32)>,
    /// 每个会话一条事件通道，关闭会话时按 id 移除。
    pub(crate) event_sender: Option<async_channel::Sender<(u64, VtEvent)>>,
    pub(crate) event_receiver: Option<async_channel::Receiver<(u64, VtEvent)>>,
    pub(crate) event_listening: bool,
    /// 网格左上角在窗口坐标系里的位置。由 `ui::grid` 在 prepaint 写入，
    /// 鼠标换算在事件回调里读取，两边不共享可变借用。
    pub(crate) grid_origin: Arc<std::sync::Mutex<Option<gpui::Point<Pixels>>>>,
    /// 光标矩形由 `ui::grid` 在 prepaint 写入，输入法据此定位候选框。
    pub(crate) cursor_slot: Arc<std::sync::Mutex<Option<Bounds<Pixels>>>>,
    /// 拖选结束写入 PRIMARY 的文本。
    pub(crate) primary_queue: Vec<String>,
    /// OSC 52 与会话复制写入的剪贴板内容，由 UI 线程落到系统剪贴板。
    pub(crate) clipboard_queue: Vec<String>,
    /// 子进程退出后请求关闭窗口（`-e` 场景）。
    pub(crate) request_quit: bool,
    /// `-e` 启动时待派生的命令会话。
    pub(crate) pending_spec: Option<SessionSpec>,
    /// 配置已改动但还没落盘的时间点，用于 500ms 防抖。
    pub(crate) dirty_since: Option<Instant>,
    /// 上次退出时保存的会话快照，启动时可据此恢复 cwd。
    pub(crate) restored: Vec<crate::io::preferences::SessionSnapshot>,
    /// `Ctrl+Shift+F` 打开的搜索条。`None` 表示未打开。
    pub(crate) search: Option<SearchState>,
    preferences_path: Option<PathBuf>,
    preferences_loaded: bool,
}

impl SiskinApp {
    pub fn new(cx: &mut gpui::Context<Self>) -> Self {
        let installed = cx.text_system().all_font_names();
        let mono_fonts = theme::mono_fonts(&installed);
        let mono = mono_fonts[0].clone();
        let ui = theme::ui_family(&installed).to_string();
        let mut settings = Settings::default();
        settings.resolve_theme(false);
        let id = 0;
        let sessions = Sessions::new(Session::starting(id, "终端", settings.shell));
        Self {
            sessions,
            focus: cx.focus_handle(),
            mono: mono.into(),
            ui: ui.into(),
            mono_fonts,
            palette: None,
            settings_section: SettingsSection::Appearance,
            inspector_open: false,
            page: Page::Terminal,
            settings,
            system_dark: false,
            menu: None,
            menu_state: MenuState::Unavailable,
            color_edit: None,
            font_picker: None,
            close_request: None,
            session_scroll: gpui::ScrollHandle::new(),
            settings_scroll: gpui::ScrollHandle::new(),
            request_focus: false,
            directory_refreshed: Instant::now(),
            edit_bounds: Arc::new(std::sync::Mutex::new(None)),
            save_error: None,
            metrics: RenderMetrics::new(8., 18., 13., 8.),
            window_title: None,
            exit_on_child_exit: false,
            status: None,
            window_id: 0,
            drag: None,
            event_sender: None,
            event_receiver: None,
            event_listening: false,
            grid_origin: Arc::new(std::sync::Mutex::new(None)),
            cursor_slot: Arc::new(std::sync::Mutex::new(None)),
            primary_queue: Vec::new(),
            clipboard_queue: Vec::new(),
            request_quit: false,
            pending_spec: None,
            dirty_since: None,
            restored: Vec::new(),
            search: None,
            preferences_path: None,
            preferences_loaded: false,
        }
    }

    pub(crate) fn theme(&self) -> theme::Palette {
        self.settings
            .palette_for(self.settings.mode, self.system_dark)
    }

    pub(crate) fn ansi(&self) -> AnsiColors {
        AnsiColors::from_palette(&self.theme())
    }

    pub(crate) fn dispatch(&mut self, action: Action) -> crate::domain::ActionEffect {
        action.run(self)
    }

    // ---- 页面与会话 ----

    pub(crate) fn session(&self) -> &Session {
        self.sessions.active()
    }

    pub(crate) fn session_mut(&mut self) -> &mut Session {
        self.sessions.active_mut()
    }

    pub(crate) fn active_index(&self) -> usize {
        self.sessions.active_index()
    }

    /// Shell 预设改动只影响新会话；正在运行的会话已经有自己的进程。
    pub(crate) fn set_shell(&mut self, kind: ShellKind) {
        self.settings.shell = kind;
        self.commit_settings();
    }

    // ---- 命令面板 ----

    pub(crate) fn open_palette(&mut self) {
        self.font_picker = None;
        self.color_edit = None;
        self.palette = Some(CommandPalette::default());
        self.request_focus = true;
    }

    pub(crate) fn close_palette(&mut self) {
        self.palette = None;
    }

    #[cfg(test)]
    pub(crate) fn palette_open(&self) -> bool {
        self.palette.is_some()
    }

    pub(crate) fn palette_entries(&self) -> Vec<&'static PaletteEntry> {
        let Some(palette) = self.palette.as_ref() else {
            return PALETTE_ENTRIES.iter().collect();
        };
        PALETTE_ENTRIES
            .iter()
            .filter(|entry| palette.matches(entry))
            .collect()
    }

    pub(crate) fn palette_type(&mut self, ch: char) {
        if let Some(palette) = self.palette.as_mut() {
            palette.type_char(ch);
        }
    }

    pub(crate) fn palette_backspace(&mut self) {
        if let Some(palette) = self.palette.as_mut() {
            palette.backspace();
        }
    }

    pub(crate) fn palette_move(&mut self, down: bool) {
        let count = self.palette_entries().len();
        if let Some(palette) = self.palette.as_mut() {
            palette.move_selection(down, count);
        }
    }

    pub(crate) fn palette_confirm(&mut self) {
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
        self.dispatch(action);
    }

    /// P0 性能采样开/关。只翻共享采样器的开关，不写终端内容、
    /// 不改会话状态；关闭时清空已采样本，避免旧数据混进新基线。
    pub(crate) fn toggle_perf(&mut self) {
        let enabled = !crate::domain::perf::is_enabled();
        crate::domain::perf::set_enabled(enabled);
    }

    // ---- 查询 ----
}

/// `SessionSpec` 只在 `-e` 启动时由 `main` 构造，这里留一个构造入口，
/// 避免把命令行参数解析塞进应用状态。
impl SiskinApp {
    pub(crate) fn with_command(mut self, spec: SessionSpec) -> Self {
        self.sessions.active_mut().title = spec.title.clone().into();
        self.pending_spec = Some(spec);
        self.exit_on_child_exit = true;
        self
    }
}
