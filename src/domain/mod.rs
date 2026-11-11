// @Time    : 2026/11/5 16:30
// @Author  : fzf
// @FileName: mod.rs
// @Software: Siskin

//! 纯数据与规则。这一层不能依赖 `gpui`、`zbus`，也不依赖上层模块。

pub(crate) mod action;
pub(crate) mod key;
pub(crate) mod metrics;
pub(crate) mod perf;
pub(crate) mod scroll;
pub(crate) mod session;
pub(crate) mod shell_marks;
pub(crate) mod shell_spec;
pub(crate) mod text_edit;

pub(crate) use action::{Action, ActionEffect};
pub(crate) use metrics::RenderMetrics;
pub(crate) use scroll::ScrollOffset;
pub(crate) use session::{ExitInfo, SessionSpec, SessionState};
pub(crate) use shell_spec::ShellKind;
