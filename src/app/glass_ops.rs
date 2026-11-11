// @Time    : 2026/11/11
// @Author  : fzf
// @FileName: glass_ops.rs
// @Software: Siskin

//! 玻璃效果开关、透度与窗口级背景同步。
//!
//! 开关与透度改动都要落盘，所以每个入口末尾都走 `commit_settings`，
//! 与 `theme_ops.rs` 的主题入口一致。

use gpui::{Window, WindowBackgroundAppearance};

use super::SiskinApp;

impl SiskinApp {
    pub(crate) fn toggle_glass(&mut self) {
        self.settings.toggle_glass();
        self.commit_settings();
    }

    pub(crate) fn cycle_glass_opacity(&mut self) {
        self.settings.cycle_glass_opacity();
        self.commit_settings();
    }

    pub(crate) fn set_glass_opacity(&mut self, opacity: f32) {
        self.settings.set_glass_opacity(opacity);
        self.commit_settings();
    }

    /// 把玻璃开关推给平台窗口：开 → 请求背后模糊，关 → 不透明。
    /// 只在变化时调用，避免每帧重复触发平台往返。
    ///
    /// `Blurred` 的实际效果看合成器：KDE Plasma Wayland 下 KWin
    /// 按 `org_kde_kwin_blur` 协议真模糊窗口背后；GNOME Wayland 与
    /// X11 上 GPUI 不发模糊请求，退化为普通透明，与玻璃关闭前的
    /// `Transparent` 表现一致（见 `docs/glass.md`）。
    pub(crate) fn sync_glass_appearance(&mut self, window: &mut Window) {
        let desired = if self.settings.glass {
            WindowBackgroundAppearance::Blurred
        } else {
            WindowBackgroundAppearance::Opaque
        };
        if self.last_glass_appearance == Some(desired) {
            return;
        }
        window.set_background_appearance(desired);
        self.last_glass_appearance = Some(desired);
    }
}
