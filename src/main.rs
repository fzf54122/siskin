// @Time    : 2026/11/4 20:14
// @Author  : fzf
// @FileName: main.rs
// @Software: Siskin

mod app;
mod dbusmenu;
mod fs;
pub mod line;
mod preferences;
mod settings;
mod shell;
mod terminal;
mod theme;
mod views;

#[cfg(test)]
mod tests;

use std::sync::Arc;

use app::SiskinApp;
use gpui::{
    App, AppContext, Bounds, TitlebarOptions, WindowBounds, WindowDecorations, WindowOptions, px,
    size,
};
use pulse_ui::application;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

/// The X11 window id the desktop panel uses to look up our menu. GPUI hands
/// us an XCB handle, and the numeric window id is exactly what the AppMenu
/// registrar expects.
fn x11_window_id(window: &gpui::Window) -> Option<u32> {
    // `Window` also has an inherent `window_handle` returning GPUI's own
    // handle type, so the raw-window-handle trait is called explicitly.
    let handle = HasWindowHandle::window_handle(window).ok()?;
    match handle.as_raw() {
        RawWindowHandle::Xcb(handle) => Some(handle.window.get()),
        RawWindowHandle::Xlib(handle) => Some(handle.window as u32),
        _ => None,
    }
}

fn window_icon() -> Option<Arc<image::RgbaImage>> {
    let bytes = include_bytes!("../assets/siskin-256.png");
    match image::load_from_memory(bytes) {
        Ok(image) => Some(Arc::new(image.to_rgba8())),
        Err(error) => {
            eprintln!("siskin: 无法解码窗口图标：{error}");
            None
        }
    }
}

fn main() {
    application().run(|cx: &mut App| {
        cx.set_app_identity("io.github.fzf.Siskin", "Siskin");
        cx.on_window_closed(|cx, _window_id| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        let bounds = Bounds::centered(None, size(px(1480.), px(920.)), cx);
        if let Err(error) = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(960.), px(640.))),
                titlebar: Some(TitlebarOptions {
                    title: Some("Siskin — simulated terminal".into()),
                    ..Default::default()
                }),
                app_id: Some("io.github.fzf.Siskin".into()),
                icon: window_icon(),
                window_decorations: Some(WindowDecorations::Server),
                ..Default::default()
            },
            |window, cx| {
                let window_id = x11_window_id(window);
                cx.new(|cx| {
                    let mut app = SiskinApp::new(cx);
                    app.set_system_dark(matches!(
                        window.appearance(),
                        gpui::WindowAppearance::Dark | gpui::WindowAppearance::VibrantDark
                    ));
                    if let Some(path) = preferences::default_path() {
                        app.load_preferences(path);
                    }
                    cx.observe_window_appearance(window, |this, window, cx| {
                        this.set_system_dark(matches!(
                            window.appearance(),
                            gpui::WindowAppearance::Dark | gpui::WindowAppearance::VibrantDark
                        ));
                        cx.notify();
                    })
                    .detach();
                    if let Some(window_id) = window_id {
                        app.install_menu(window_id, cx);
                    }
                    app.focus.focus(window, cx);
                    app
                })
            },
        ) {
            eprintln!("siskin: 无法打开窗口：{error}。请检查桌面会话和 Vulkan 驱动。");
            cx.quit();
            return;
        }
        cx.activate(true);
    });
}
