// @Time    : 2026/11/4 20:14
// @Author  : fzf
// @FileName: main.rs
// @Software: Siskin

mod app;
mod fs;
pub mod line;
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
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        let bounds = Bounds::centered(None, size(px(1240.), px(800.)), cx);
        if let Err(error) = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(900.), px(560.))),
                titlebar: Some(TitlebarOptions {
                    title: Some("Siskin — simulated terminal".into()),
                    ..Default::default()
                }),
                app_id: Some("io.github.fzf.Siskin".into()),
                icon: window_icon(),
                window_decorations: Some(WindowDecorations::Client),
                ..Default::default()
            },
            |window, cx| {
                cx.new(|cx| {
                    let app = SiskinApp::new(cx);
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
