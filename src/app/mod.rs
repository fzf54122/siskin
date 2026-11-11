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
pub(crate) mod events;
mod keys;
mod menu_ops;
mod palette;
pub(crate) mod pane;
mod pane_ops;
mod prefs_ops;
mod search_ops;
pub(crate) mod session;
mod shell_marks;
mod tab_ops;
mod terminal_ops;
mod theme_ops;
mod workspace_ops;

pub(crate) use actions::{PALETTE_ENTRIES, PaletteEntry};
pub(crate) use close_ops::CloseRequest;
pub(crate) use editor_ops::FontPicker;
pub(crate) use keys::Handled;
pub(crate) use palette::CommandPalette;
pub(crate) use pane::{PaneDrag, SplitDirection};
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
    /// 当前聚焦的分屏叶（会话 id）。`session()` / `session_mut()`
    /// 都指向它，键盘输入、滚动、搜索天然落在焦点 pane 上。
    pub active_pane: u64,
    /// 每个标签上次聚焦的叶，切换标签回来时恢复到它。
    pub(crate) pane_focus: std::collections::HashMap<u64, u64>,
    /// 活动标签每叶的窗口矩形。`resize_panes` 每帧按树重算，
    /// 鼠标换算与分隔线拖动都读它，不读共享的 `grid_origin`。
    pub(crate) pane_bounds: std::collections::HashMap<u64, Bounds<Pixels>>,
    /// 活动标签整块终端区的窗口矩形（tab 条下方），分屏渲染的总框。
    pub(crate) pane_area: Bounds<Pixels>,
    /// 分隔线拖动状态；与文本选区的 `drag` 是两个字段，不共用。
    pub(crate) pane_drag: Option<PaneDrag>,
    /// 滚动条 thumb 拖动状态；拖动中指针离开细条也继续处理。
    pub(crate) scrollbar_drag: Option<crate::ui::scrollbar::ScrollbarDrag>,
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
    pub(crate) drag: Option<(isize, usize)>,
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
    /// 标签拖拽状态（手动鼠标方案，见 `tab_ops.rs`）。
    pub(crate) tab_drag: Option<tab_ops::TabDrag>,
    /// 拖拽换过位后，松开那下合成的 click 要吞掉，不选标签。
    pub(crate) tab_click_suppressed: bool,
    /// 正在内联重命名的标签。`None` 表示没有在改名。
    pub(crate) tab_rename: Option<tab_ops::TabRename>,
    preferences_path: Option<PathBuf>,
    preferences_loaded: bool,
}

impl SiskinApp {
    pub fn new(cx: &mut gpui::Context<Self>) -> Self {
        let installed = cx.text_system().all_font_names();
        let mono_fonts = theme::mono_fonts(&installed);
        let mut settings = Settings::default();
        settings.resolve_theme(false);
        let sessions = Sessions::new(Session::starting(0, "终端", settings.shell));
        Self {
            sessions,
            active_pane: 0,
            pane_focus: std::collections::HashMap::new(),
            pane_bounds: std::collections::HashMap::new(),
            pane_area: Self::initial_pane_area(),
            pane_drag: None,
            scrollbar_drag: None,
            focus: cx.focus_handle(),
            mono: mono_fonts[0].clone().into(),
            ui: theme::ui_family(&installed).to_string().into(),
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
            tab_drag: None,
            tab_click_suppressed: false,
            tab_rename: None,
            preferences_path: None,
            preferences_loaded: false,
        }
    }

    /// 首帧 `refresh_metrics` 之前的分屏总框；第一帧就会被真实视口重算。
    fn initial_pane_area() -> Bounds<Pixels> {
        Bounds::new(
            gpui::point(gpui::px(0.), gpui::px(28.)),
            gpui::size(gpui::px(1024.), gpui::px(768.)),
        )
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

    /// 焦点 pane 的会话；`active_pane` 失效时退回活动标签，
    /// 这样旧的调用点在分屏数据还没建好时也不会取到空引用。
    pub(crate) fn session(&self) -> &Session {
        match self.session_by_id(self.active_pane) {
            Some(session) => session,
            None => self.sessions.active(),
        }
    }

    pub(crate) fn session_mut(&mut self) -> &mut Session {
        // 下标在 `Sessions::focused_mut` 里按 id 校验、取不到退回活动标签，
        // 这样旧的调用点在分屏数据还没建好时也不会取空引用。
        self.sessions.focused_mut(self.active_pane)
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
