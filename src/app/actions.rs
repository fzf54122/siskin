// @Time    : 2026/11/5 17:05
// @Author  : fzf
// @FileName: actions.rs
// @Software: Siskin

//! 动作执行。菜单、命令面板、快捷键只产出 `Action`，真正的分支全部收敛
//! 在 `Action::run`；新增动作只需加枚举项和一条臂。

use crate::app::SiskinApp;
use crate::domain::shell_spec::ShellKind;
use crate::domain::{Action, ActionEffect};
use crate::theme::ThemeId;

impl Action {
    pub(crate) fn run(self, app: &mut SiskinApp) -> ActionEffect {
        // 命令面板的“最近操作”从这里统一记：菜单、快捷键、面板
        // 三处入口都走 `run`，打开面板本身不记。
        if self != Action::OpenPalette {
            app.record_recent(self);
        }
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
            Action::ToggleAgentSidebar => app.toggle_agent_sidebar(),
            Action::OpenSettings => app.set_page(crate::settings::Page::Settings),
            Action::OpenTerminal => app.set_page(crate::settings::Page::Terminal),
            Action::CycleThemeMode => app.cycle_theme(),
            Action::SetTheme(theme) => app.set_theme(theme),
            Action::FontLarger => app.step_font(true),
            Action::FontSmaller => app.step_font(false),
            Action::ToggleInspector => app.toggle_inspector(),
            Action::NextSession => app.cycle_session(true),
            Action::PreviousSession => app.cycle_session(false),
            Action::SplitHorizontal => app.split_pane(crate::app::SplitDirection::Horizontal),
            Action::SplitVertical => app.split_pane(crate::app::SplitDirection::Vertical),
            Action::FocusNextPane => app.focus_pane_cycle(true),
            Action::FocusPrevPane => app.focus_pane_cycle(false),
            Action::ClosePane => {
                let id = app.active_pane;
                app.close_pane(id)
            }
            Action::SetShell(kind) => app.set_shell(kind),
            Action::ScrollPageUp => app.scroll_page(true),
            Action::ScrollPageDown => app.scroll_page(false),
            Action::ScrollToTop => app.scroll_to_top(),
            Action::ScrollToBottom => app.scroll_to_bottom(),
            Action::NextCommand => app.jump_command(true),
            Action::PreviousCommand => app.jump_command(false),
            Action::OpenSearch => app.open_search(),
            Action::TogglePerf => app.toggle_perf(),
            Action::ToggleGlass => app.toggle_glass(),
            Action::CycleGlassOpacity => app.cycle_glass_opacity(),
            Action::RenameSession => app.rename_active_tab(),
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

pub(crate) static PALETTE_ENTRIES: [PaletteEntry; 39] = [
    PaletteEntry {
        label: "玻璃效果开/关",
        keywords: "glass blur frosted transparency glassmorphism 磨砂 玻璃 模糊 透明",
        hint: "浮层用半透明磨砂底，关闭回到实色；终端主区始终实底",
        keys: "Ctrl Shift G",
        action: Action::ToggleGlass,
    },
    PaletteEntry {
        label: "玻璃透度",
        keywords: "glass opacity transparency 磨砂 透度 不透明度",
        hint: "标准 / 较浓 / 浓郁三档循环，均按 4.5:1 校验过对比度",
        keys: "",
        action: Action::CycleGlassOpacity,
    },
    PaletteEntry {
        label: "Agent 侧栏",
        keywords: "agent sidebar session status agent 侧栏 会话 状态",
        hint: "按会话列出当前命令与状态，点击条目聚焦到对应会话",
        keys: "Ctrl Shift A",
        action: Action::ToggleAgentSidebar,
    },
    PaletteEntry {
        label: "重命名当前标签",
        keywords: "rename tab session title 重命名 标签 改名",
        hint: "双击标签也可重命名；回车确认，Esc 取消，清空恢复默认名",
        keys: "",
        action: Action::RenameSession,
    },
    PaletteEntry {
        label: "下一个命令",
        keywords: "next command shell jump osc133",
        hint: "跳到下一条命令的起始行（需 shell 发 OSC 133 标记）",
        keys: "Ctrl Shift Down",
        action: Action::NextCommand,
    },
    PaletteEntry {
        label: "上一个命令",
        keywords: "previous command shell jump osc133",
        hint: "跳到上一条命令的起始行（需 shell 发 OSC 133 标记）",
        keys: "Ctrl Shift Up",
        action: Action::PreviousCommand,
    },
    PaletteEntry {
        label: "向下分屏",
        keywords: "split pane horizontal below down",
        hint: "上下分屏，新 pane 在下方",
        keys: "Ctrl Shift D",
        action: Action::SplitHorizontal,
    },
    PaletteEntry {
        label: "向右分屏",
        keywords: "split pane vertical right",
        hint: "左右分屏，新 pane 在右方",
        keys: "Ctrl Shift R",
        action: Action::SplitVertical,
    },
    PaletteEntry {
        label: "下一个分屏",
        keywords: "next pane focus split",
        hint: "焦点移到下一个 pane",
        keys: "Ctrl Shift J",
        action: Action::FocusNextPane,
    },
    PaletteEntry {
        label: "上一个分屏",
        keywords: "previous pane focus split",
        hint: "焦点移到上一个 pane",
        keys: "Ctrl Shift K",
        action: Action::FocusPrevPane,
    },
    PaletteEntry {
        label: "关闭当前分屏",
        keywords: "close pane split kill",
        hint: "只关当前 pane，留一个时关整个标签",
        keys: "Ctrl Shift Q",
        action: Action::ClosePane,
    },
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
        label: "向下翻页",
        keywords: "scroll page down history",
        hint: "向新输出方向翻一页",
        keys: "Shift PageDown",
        action: Action::ScrollPageDown,
    },
    PaletteEntry {
        label: "滚到最旧输出",
        keywords: "scroll top oldest history",
        hint: "滚到滚动区的最顶端",
        keys: "Shift Home",
        action: Action::ScrollToTop,
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
        label: "回到终端",
        keywords: "terminal page back open",
        hint: "从设置页回到终端",
        keys: "Ctrl 1",
        action: Action::OpenTerminal,
    },
    PaletteEntry {
        label: "切换配色主题",
        keywords: "theme palette color dark light",
        hint: "跟随系统 / 浅色 / 深色 / 自定义",
        keys: "Ctrl Shift T",
        action: Action::CycleThemeMode,
    },
    PaletteEntry {
        label: "主题：Paper",
        keywords: "theme paper light white",
        hint: "暖白纸面",
        keys: "",
        action: Action::SetTheme(ThemeId::Paper),
    },
    PaletteEntry {
        label: "主题：Graphite",
        keywords: "theme graphite dark gray",
        hint: "中性深灰",
        keys: "",
        action: Action::SetTheme(ThemeId::Graphite),
    },
    PaletteEntry {
        label: "主题：Moss",
        keywords: "theme moss light green",
        hint: "苔绿浅色",
        keys: "",
        action: Action::SetTheme(ThemeId::Moss),
    },
    PaletteEntry {
        label: "主题：Dusk",
        keywords: "theme dusk dark indigo blue",
        hint: "靛蓝夜色",
        keys: "",
        action: Action::SetTheme(ThemeId::Dusk),
    },
    PaletteEntry {
        label: "主题：Catppuccin Mocha",
        keywords: "theme catppuccin mocha dark pastel mauve",
        hint: "粉彩摩卡",
        keys: "",
        action: Action::SetTheme(ThemeId::CatppuccinMocha),
    },
    PaletteEntry {
        label: "主题：Tokyo Night",
        keywords: "theme tokyo night dark blue",
        hint: "东京夜蓝",
        keys: "",
        action: Action::SetTheme(ThemeId::TokyoNight),
    },
    PaletteEntry {
        label: "放大字号",
        keywords: "font bigger larger zoom",
        hint: "终端等宽字号放大一档",
        keys: "Ctrl +",
        action: Action::FontLarger,
    },
    PaletteEntry {
        label: "缩小字号",
        keywords: "font smaller zoom",
        hint: "终端等宽字号缩小一档",
        keys: "Ctrl -",
        action: Action::FontSmaller,
    },
    PaletteEntry {
        label: "性能采样开/关",
        keywords: "performance perf fps baseline metrics",
        hint: "显示 FPS、P95、整形与构帧耗时，可复现基线用",
        keys: "",
        action: Action::TogglePerf,
    },
    PaletteEntry {
        label: "默认 Shell：zsh",
        keywords: "shell default zsh terminal",
        hint: "只影响之后新建的会话",
        keys: "",
        action: Action::SetShell(ShellKind::Zsh),
    },
    PaletteEntry {
        label: "默认 Shell：bash",
        keywords: "shell default bash terminal",
        hint: "只影响之后新建的会话",
        keys: "",
        action: Action::SetShell(ShellKind::Bash),
    },
    PaletteEntry {
        label: "默认 Shell：fish",
        keywords: "shell default fish terminal",
        hint: "只影响之后新建的会话",
        keys: "",
        action: Action::SetShell(ShellKind::Fish),
    },
    PaletteEntry {
        label: "默认 Shell：sh",
        keywords: "shell default sh terminal",
        hint: "只影响之后新建的会话",
        keys: "",
        action: Action::SetShell(ShellKind::Sh),
    },
    PaletteEntry {
        label: "退出 Siskin",
        keywords: "quit exit close app",
        hint: "退出应用",
        keys: "",
        action: Action::Quit,
    },
];
