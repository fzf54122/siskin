// @Time    : 2026/11/4 20:14
// @Author  : fzf
// @FileName: main.rs
// @Software: Siskin

// 生产路径不 panic：`unwrap` / `expect` / `panic!` 只允许出现在测试里。
#![cfg_attr(
    not(test),
    deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)
)]

mod app;
mod domain;
mod io;
mod settings;
mod theme;
mod ui;

#[cfg(test)]
mod tests;

use std::path::PathBuf;
use std::sync::Arc;

use app::SiskinApp;
use domain::SessionSpec;
use gpui::{
    App, AppContext, Bounds, TitlebarOptions, WindowBounds, WindowDecorations, WindowOptions, px,
    size,
};
use pulse_ui::application;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

/// 命令行参数。只保留终端惯例里的三项，不做通用参数框架。
#[derive(Default)]
struct Args {
    working_directory: Option<PathBuf>,
    /// `-e <program> [args...]`：执行命令，退出即关窗。
    command: Option<Vec<String>>,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args::default();
    let mut values = std::env::args().skip(1);
    while let Some(value) = values.next() {
        match value.as_str() {
            "--working-directory" | "-d" => {
                let directory = values.next().ok_or("--working-directory 缺少路径")?;
                args.working_directory = Some(PathBuf::from(directory));
            }
            "-e" | "--command" => {
                let command: Vec<String> = values.by_ref().collect();
                if command.is_empty() {
                    return Err("-e 后面需要命令".to_string());
                }
                args.command = Some(command);
            }
            "--version" | "-V" => {
                println!("siskin {}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            "--help" | "-h" => {
                println!(
                    "siskin {}\n\n用法：siskin [选项] [-- -e 命令 [参数...]]\n\n  -d, --working-directory <目录>  新会话的起始目录\n  -e, --command <命令> [参数...]  执行命令，退出即关闭窗口\n  -V, --version                   打印版本\n",
                    env!("CARGO_PKG_VERSION")
                );
                std::process::exit(0);
            }
            other if other.starts_with('-') => {
                return Err(format!("未知参数：{other}"));
            }
            other => {
                args.working_directory = Some(PathBuf::from(other));
            }
        }
    }
    Ok(args)
}

fn session_spec(args: &Args) -> Option<SessionSpec> {
    let command = args.command.as_ref()?;
    let program = PathBuf::from(&command[0]);
    let rest = command[1..].to_vec();
    Some(
        SessionSpec::command(command[0].clone(), program, rest)
            .with_directory(args.working_directory.clone()),
    )
}

/// The X11 window id the desktop panel uses to look up our menu. GPUI hands
/// us an XCB handle, and the numeric window id is exactly what the AppMenu
/// registrar expects.
fn x11_window_id(window: &gpui::Window) -> Option<u32> {
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

fn window_options(cx: &mut App) -> WindowOptions {
    let bounds = Bounds::centered(None, size(px(1480.), px(920.)), cx);
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        window_min_size: Some(size(px(960.), px(640.))),
        titlebar: Some(TitlebarOptions {
            title: Some("Siskin".into()),
            ..Default::default()
        }),
        app_id: Some("io.github.fzf.Siskin".into()),
        icon: window_icon(),
        window_decorations: Some(WindowDecorations::Server),
        ..Default::default()
    }
}

fn build_app(
    window: &mut gpui::Window,
    cx: &mut gpui::Context<SiskinApp>,
    spec: Option<SessionSpec>,
) -> SiskinApp {
    let window_id = x11_window_id(window);
    let mut app = SiskinApp::new(cx);
    app.set_system_dark(matches!(
        window.appearance(),
        gpui::WindowAppearance::Dark | gpui::WindowAppearance::VibrantDark
    ));
    if let Some(path) = io::preferences::default_path() {
        app.load_preferences(path);
    }
    if let Some(spec) = spec {
        app = app.with_command(spec);
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
    app.start_sessions(cx);
    let view = cx.entity().downgrade();
    window.on_window_should_close(cx, move |_, cx| {
        view.update(cx, |app, cx| {
            app.request_exit();
            if app.request_quit {
                app.flush_preferences();
            }
            cx.notify();
            app.request_quit
        })
        .unwrap_or(true)
    });
    app
}

/// 单实例判定的三种结果。
enum Startup {
    /// 已有实例接收了参数，本进程直接退出。
    Forwarded,
    /// 本进程是主实例，可以打开窗口。
    Owner(Option<io::instance::InstanceServer>),
}

/// `-e` 每次都要独立执行，因此不参与转发。
fn claim_instance(args: &Args) -> Startup {
    if args.command.is_some() {
        return Startup::Owner(None);
    }
    match io::instance::claim(
        args.working_directory
            .as_ref()
            .map(|path| path.display().to_string()),
    ) {
        io::instance::Secondary::Forwarded => Startup::Forwarded,
        io::instance::Secondary::Owner(server) => Startup::Owner(Some(server)),
    }
}

fn main() {
    let args = match parse_args() {
        Ok(args) => args,
        Err(message) => {
            eprintln!("siskin: {message}");
            std::process::exit(2);
        }
    };
    let instance = match claim_instance(&args) {
        Startup::Forwarded => return,
        Startup::Owner(server) => server,
    };

    // 闭包需要 `'static`，把命令会话的所有权先移进一个只读句柄。
    let spec = Arc::new(session_spec(&args));
    let instance = Arc::new(instance);

    application().run(move |cx: &mut App| {
        // 持有连接，drop 时自动释放 D-Bus 名字。
        let _instance = instance.clone();
        cx.set_app_identity("io.github.fzf.Siskin", "Siskin");
        cx.on_window_closed(|cx, _window_id| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        let options = window_options(cx);
        let spec_for_window = spec.clone();
        let opened = cx.open_window(options, move |window, cx| {
            let spec = spec_for_window.as_ref().clone();
            cx.new(|cx| build_app(window, cx, spec))
        });
        let window_handle = match opened {
            Ok(handle) => handle,
            Err(error) => {
                eprintln!("siskin: 无法打开窗口：{error}。请检查桌面会话和 Vulkan 驱动。");
                cx.quit();
                return;
            }
        };
        // 退出前把防抖窗口内的改动写盘，避免最后一次设置丢失。
        let pending = window_handle
            .entity(cx)
            .map(|entity| entity.downgrade())
            .ok();
        cx.on_app_quit(move |cx| {
            if let Some(entity) = pending.as_ref() {
                let _ = entity.update(cx, |app, _| app.flush_preferences());
            }
            async {}
        })
        .detach();
        cx.activate(true);
    });
}
