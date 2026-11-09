// @Time    : 2026/11/5 16:32
// @Author  : fzf
// @FileName: action.rs
// @Software: Siskin

//! 用户动作。菜单、命令面板、快捷键三处入口都产出这个枚举，执行在
//! `app/actions.rs`。放在 domain 是为了让 `io::dbusmenu` 能直接产出动作，
//! 而不必反向依赖应用层。

use crate::domain::shell_spec::ShellKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Action {
    NewSession,
    CloseSession,
    Clear,
    Quit,
    OpenPalette,
    OpenSettings,
    OpenTerminal,
    CycleThemeMode,
    FontLarger,
    FontSmaller,
    ToggleInspector,
    NextSession,
    PreviousSession,
    SetShell(ShellKind),
    ScrollPageUp,
    ScrollPageDown,
    ScrollToTop,
    ScrollToBottom,
    OpenSearch,
}

/// 动作的副作用范围。图形层据此决定刷新窗口还是退出。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ActionEffect {
    Redraw,
    Quit,
}
