// @Time    : 2026/11/5 17:05
// @Author  : fzf
// @FileName: actions.rs
// @Software: Siskin

//! 动作执行。菜单、命令面板、快捷键只产出 `Action`，真正的分支全部收敛
//! 在 `Action::run`；新增动作只需加枚举项和一条臂。

use crate::app::SiskinApp;
use crate::domain::{Action, ActionEffect};

impl Action {
    pub(crate) fn run(self, app: &mut SiskinApp) -> ActionEffect {
        match self {
            Action::NewSession => app.new_session(),
            Action::CloseSession => {
                let index = app.active_index();
                app.close_session(index)
            }
            Action::Clear => app.clear_screen(),
            Action::Quit => {
                app.request_exit();
                return if app.request_quit {
                    ActionEffect::Quit
                } else {
                    ActionEffect::Redraw
                };
            }
            Action::OpenPalette => app.open_palette(),
            Action::OpenSettings => app.set_page(crate::settings::Page::Settings),
            Action::OpenTerminal => app.set_page(crate::settings::Page::Terminal),
            Action::CycleThemeMode => app.cycle_theme(),
            Action::FontLarger => app.step_font(true),
            Action::FontSmaller => app.step_font(false),
            Action::ToggleInspector => app.toggle_inspector(),
            Action::NextSession => app.cycle_session(true),
            Action::PreviousSession => app.cycle_session(false),
            Action::SetShell(kind) => app.set_shell(kind),
            Action::ScrollPageUp => app.scroll_page(true),
            Action::ScrollPageDown => app.scroll_page(false),
            Action::ScrollToTop => app.scroll_to_top(),
            Action::ScrollToBottom => app.scroll_to_bottom(),
            Action::OpenSearch => app.open_search(),
            Action::TogglePerf => app.toggle_perf(),
        }
        app.sync_menu();
        ActionEffect::Redraw
    }
}

/// 命令面板条目。条目只负责展示与过滤，执行统一走 `Action`。
pub(crate) struct PaletteEntry {
    pub label: &'static str,
    /// English keywords so both "复制" and "copy" find the same entry.
    pub keywords: &'static str,
    pub hint: &'static str,
    pub keys: &'static str,
    pub action: Action,
}

pub(crate) static PALETTE_ENTRIES: [PaletteEntry; 13] = [
    PaletteEntry {
        label: "新建会话",
        keywords: "new session tab open",
        hint: "启动一个新的 shell",
        keys: "Ctrl Shift N",
        action: Action::NewSession,
    },
    PaletteEntry {
        label: "关闭当前会话",
        keywords: "close session kill exit",
        hint: "结束当前 shell 进程",
        keys: "Ctrl Shift W",
        action: Action::CloseSession,
    },
    PaletteEntry {
        label: "清屏",
        keywords: "clear screen scrollback",
        hint: "清掉可视区与滚动区，shell 状态不变",
        keys: "",
        action: Action::Clear,
    },
    PaletteEntry {
        label: "搜索滚动区",
        keywords: "search find scrollback",
        hint: "高亮匹配并逐个跳转",
        keys: "Ctrl Shift F",
        action: Action::OpenSearch,
    },
    PaletteEntry {
        label: "向上翻页",
        keywords: "scroll page up history",
        hint: "回看滚动区",
        keys: "Shift PageUp",
        action: Action::ScrollPageUp,
    },
    PaletteEntry {
        label: "回到最新输出",
        keywords: "scroll bottom latest",
        hint: "滚回底部并跟随输出",
        keys: "Shift End",
        action: Action::ScrollToBottom,
    },
    PaletteEntry {
        label: "下一会话",
        keywords: "next session switch tab",
        hint: "在左侧会话之间切换",
        keys: "Ctrl Tab",
        action: Action::NextSession,
    },
    PaletteEntry {
        label: "上一会话",
        keywords: "previous session switch tab",
        hint: "在左侧会话之间切换",
        keys: "Ctrl Shift Tab",
        action: Action::PreviousSession,
    },
    PaletteEntry {
        label: "切换会话信息",
        keywords: "inspector panel session information",
        hint: "显示或收起右侧会话信息",
        keys: "Ctrl Shift I",
        action: Action::ToggleInspector,
    },
    PaletteEntry {
        label: "打开设置",
        keywords: "settings preferences theme font",
        hint: "主题、字体、shell",
        keys: "Ctrl ,",
        action: Action::OpenSettings,
    },
    PaletteEntry {
        label: "切换配色主题",
        keywords: "theme palette color dark light",
        hint: "跟随系统 / 浅色 / 深色 / 自定义",
        keys: "Ctrl Shift T",
        action: Action::CycleThemeMode,
    },
    PaletteEntry {
        label: "放大/缩小字号",
        keywords: "font bigger smaller zoom",
        hint: "终端等宽字号按档位调整",
        keys: "Ctrl +/-",
        action: Action::FontLarger,
    },
    PaletteEntry {
        label: "性能采样开/关",
        keywords: "performance perf fps baseline metrics",
        hint: "显示 FPS、P95、整形与构帧耗时，可复现基线用",
        keys: "",
        action: Action::TogglePerf,
    },
];
