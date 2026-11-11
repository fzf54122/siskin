// @Time    : 2026/11/5 16:25
// @Author  : fzf
// @FileName: mod.rs
// @Software: Siskin

//! 系统边界。可以依赖 `serde`、`zbus`、`alacritty_terminal`，不能依赖 `gpui`。

pub(crate) mod ansi;
pub(crate) mod dbusmenu;
pub(crate) mod history_search;
pub(crate) mod instance;
pub(crate) mod keys;
pub(crate) mod links;
pub(crate) mod osc133;
pub(crate) mod preferences;
pub(crate) mod pty;
pub(crate) mod pty_channel;
pub(crate) mod pty_read;
pub(crate) mod search;
pub(crate) mod vt;
