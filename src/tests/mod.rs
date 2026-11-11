// @Time    : 2026/11/5 13:10
// @Author  : fzf
// @FileName: mod.rs
// @Software: Siskin

//! 跨层测试：视图渲染、按键链路、会话仓储、真实 PTY。菜单协议测试贴着
//! `io::dbusmenu` 放，因为 `MenuService` 的接口方法只在该模块内可见。

mod agent_sidebar;
mod compat;
mod compat_gui;
mod compat_pty;
mod ghost;
mod keys;
mod layout;
mod palette;
mod panes;
mod preferences;
mod pty;
mod pty_drain;
mod pty_osc133;
mod search_history;
mod session;
mod tabs;
mod views;
