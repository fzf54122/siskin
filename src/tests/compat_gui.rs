// @Time    : 2026/11/11
// @Author  : Lux
// @FileName: compat_gui.rs
// @Software: Siskin

//! P4 兼容性测试：GPUI 端到端。OSC 8 白名单外的点击拦截、鼠标报告
//! 经真实 PTY 到 `cat` 的文件回显。
//!
//! 测试窗口的初始会话没有 PTY（`start_sessions` 只在 `main` 里调），
//! 所以先点 `new-session` 再操作；序列由 shell 的 `printf` 输出
//! （zsh/sh 的 `\033` 转义已验证），不依赖行规程回显——交互式
//! shell 的行编辑会吃掉回显的转义序列，且 `ECHOCTL` 会把 ESC
//! 变成可见的 `^[`。

use std::time::{Duration, Instant};

use alacritty_terminal::term::TermMode;
use gpui::{Entity, TestAppContext, VisualTestContext};

use crate::app::SiskinApp;

use super::views::{click, read, window};

/// `wait_until` 只等 2 秒；并行负载下 PTY 启动与 `cat` 回写可能更慢，
/// 这里给 15 秒，超时由调用方的断言报出。
///
/// 每轮先手动 `drain_events()`：测试窗口不走 `main` 的
/// `start_sessions`，`listen_pty_events` 的定时轮询从不启动，
/// 不手动排空帧快照永远停在首帧。
fn poll_until(
    view: &Entity<SiskinApp>,
    cx: &mut VisualTestContext,
    predicate: impl Fn(&SiskinApp) -> bool,
) -> bool {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        view.update(cx, |app, _| {
            let _ = app.drain_events();
        });
        if cx.update(|_, app| predicate(view.read(app))) {
            return true;
        }
        if Instant::now() > deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// 新建一个带真实 PTY 的会话并切到它。返回时 PTY 已派生，
/// shell 可能还在启动，输入会进 PTY 缓冲，按序执行。
fn new_pty_session(cx: &mut TestAppContext) -> (Entity<SiskinApp>, &mut VisualTestContext) {
    let (view, cx) = window(cx);
    click(cx, "new-session");
    assert!(
        poll_until(&view, cx, |app| app.sessions.len() == 2),
        "点 new-session 后会话数未变成 2"
    );
    assert!(
        poll_until(&view, cx, |app| app.active_index() == 1),
        "新建会话后焦点未切到新会话"
    );
    assert!(
        poll_until(&view, cx, |app| app.session().pty().is_some()),
        "新建会话没有 PTY"
    );
    (view, cx)
}

fn write_session(view: &Entity<SiskinApp>, cx: &mut VisualTestContext, bytes: &[u8]) {
    let bytes = bytes.to_vec();
    view.update(cx, |app, _| app.session_mut().write(&bytes));
}

fn frame_text(app: &SiskinApp) -> String {
    app.session()
        .frame
        .rows
        .iter()
        .map(|row| {
            row.cells
                .iter()
                .map(|cell| cell.text.as_str())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn find_link(app: &SiskinApp, uri: &str) -> Option<(isize, usize)> {
    for (row, line) in app.session().frame.rows.iter().enumerate() {
        for (column, cell) in line.cells.iter().enumerate() {
            if cell.link.as_deref() == Some(uri) {
                return Some((row as isize, column));
            }
        }
    }
    None
}

#[gpui::test]
fn osc8_non_whitelisted_scheme_is_not_clickable(cx: &mut TestAppContext) {
    let (view, cx) = new_pty_session(cx);
    write_session(
        &view,
        cx,
        b"printf '\\033]8;;javascript:alert(1)\\033\\\\clickme\\033]8;;\\033\\\\\\n'\n",
    );
    assert!(
        poll_until(&view, cx, |app| find_link(app, "javascript:alert(1)")
            .is_some()),
        "OSC 8 链接未出现在帧里，帧内容：\n{}",
        read(&view, cx, frame_text)
    );
    let (row, column) = read(&view, cx, |app| {
        find_link(app, "javascript:alert(1)").expect("链接应已在帧里")
    });
    let opened = view.update(cx, |app, _| app.open_link_at(row, column));
    assert!(!opened, "白名单外的 scheme 不能打开");
    let status = read(&view, cx, |app| app.status.clone());
    assert!(
        status.as_ref().is_some_and(|text| text.contains("已拦截")),
        "应给出拦截提示，实际 {status:?}"
    );
}

/// 经真实 PTY 验证一次鼠标报告：程序开模式 → `forward_mouse` →
/// `cat` 把收到的字节原样写进文件，测试直接读文件断言精确字节。
/// 不走 `od` / 回显：交互式 shell 的行编辑会恢复 `stty -echo`，
/// 回显也传不了带控制字符的报告序列。
fn mouse_report_reaches_file(
    cx: &mut TestAppContext,
    enable: &str,
    want_sgr: bool,
    expected: &[u8],
) {
    let (view, cx) = new_pty_session(cx);
    write_session(&view, cx, format!("printf '{enable}'\n").as_bytes());
    assert!(
        poll_until(&view, cx, |app| {
            app.session().pty().is_some_and(|pty| {
                let mode = pty.mode();
                if want_sgr {
                    mode.contains(TermMode::SGR_MOUSE)
                } else {
                    mode.contains(TermMode::MOUSE_REPORT_CLICK)
                        && !mode.contains(TermMode::SGR_MOUSE)
                }
            })
        }),
        "程序开鼠标模式后，终端模式位未就绪"
    );
    let path = std::env::temp_dir().join(format!(
        "siskin-p4-mouse-{}-{}.bin",
        std::process::id(),
        if want_sgr { "sgr" } else { "x10" }
    ));
    let _ = std::fs::remove_file(&path);
    write_session(&view, cx, format!("cat > {}\n", path.display()).as_bytes());
    let id = read(&view, cx, |app| app.session().id);
    // 列 5、行 3 的按下。
    let reported = view.update(cx, |app, _| app.forward_mouse(id, 2, 4, 0, true, false));
    assert!(reported, "鼠标报告应被转发");
    // `\n` 封行：行规程把报告字节整行交给 cat。不能用 Ctrl-D——
    // 交互式 shell 下子进程的 termios 里 ISIG 常是关的，Ctrl-D
    // 会被当成普通字节写进文件（实测会多出两个 0x04）。
    write_session(&view, cx, b"\n");
    wait_for_mouse_file(&path, expected);
    let _ = std::fs::remove_file(&path);
    if want_sgr {
        let shifted = view.update(cx, |app, _| app.forward_mouse(id, 2, 4, 0, true, true));
        assert!(!shifted, "Shift 按下时鼠标事件留给本地选区，不上报");
    }
}

/// 等 `cat` 写出的文件出现：报告字节本身必须精确，行尾是 `\n` 或 `\r`
/// 其一（交互式 shell 的行编辑在某些时序下会让行规程按回车提交这一行）。
fn wait_for_mouse_file(path: &std::path::Path, expected: &[u8]) {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if let Ok(bytes) = std::fs::read(path)
            && bytes.len() == expected.len() + 1
            && bytes[..expected.len()] == *expected
            && matches!(bytes[expected.len()], b'\n' | b'\r')
        {
            return;
        }
        if Instant::now() > deadline {
            let actual = std::fs::read(path).unwrap_or_default();
            panic!("cat 收到的鼠标报告不是预期字节：实际 {actual:?}，预期 {expected:?} 加行尾");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[gpui::test]
fn sgr_mouse_report_reaches_the_program(cx: &mut TestAppContext) {
    // ESC [ < 0 ; 5 ; 3 M
    mouse_report_reaches_file(cx, "\\033[?1000h\\033[?1006h", true, b"\x1b[<0;5;3M");
}

#[gpui::test]
fn x10_mouse_report_reaches_the_program(cx: &mut TestAppContext) {
    // ESC [ M SP % #
    mouse_report_reaches_file(cx, "\\033[?1000h", false, &[0x1b, b'[', b'M', 32, 37, 35]);
}

#[gpui::test]
fn ctrl_d_on_empty_prompt_closes_the_session(cx: &mut TestAppContext) {
    let (view, cx) = new_pty_session(cx);
    // 等 shell 就绪：帧里出现非空内容即已到提示符。不认提示符字符本身——
    // root 下 zsh 提示符是 `#`，普通用户是 `%`，写死任一都会误判。
    assert!(
        poll_until(&view, cx, |app| { !frame_text(app).trim().is_empty() }),
        "shell 15 秒内未输出提示符"
    );
    // 先 Ctrl+C 清掉提示行上可能残留的输入，再 Ctrl+D：空提示符的
    // EOF 让 shell 退出；连按几次也无妨（zsh 的 ignoreeof 只挡有限次）。
    for _ in 0..3 {
        write_session(&view, cx, b"\x03\x04");
        std::thread::sleep(Duration::from_millis(200));
    }
    assert!(
        poll_until(&view, cx, |app| { app.sessions.len() == 1 }),
        "Ctrl+D 后退出的标签 15 秒内未自动关闭"
    );
    assert_eq!(read(&view, cx, |app| { app.active_index() }), 0);
}

#[gpui::test]
fn ctrl_d_on_nonempty_prompt_does_not_close_the_session(cx: &mut TestAppContext) {
    let (view, cx) = new_pty_session(cx);
    // 帧里出现非空内容即已到提示符（root 下是 `#`，普通用户是 `%`）。
    assert!(
        poll_until(&view, cx, |app| { !frame_text(app).trim().is_empty() }),
        "shell 15 秒内未输出提示符"
    );
    write_session(&view, cx, b"abc");
    assert!(
        poll_until(&view, cx, |app| { frame_text(app).contains("abc") }),
        "输入回显 15 秒内未出现"
    );
    write_session(&view, cx, b"\x04");
    // 排空事件，确认会话还在：EOF 只是 shell 行编辑删一个字符。
    view.update(cx, |app, _| {
        let _ = app.drain_events();
    });
    std::thread::sleep(Duration::from_millis(500));
    view.update(cx, |app, _| {
        let _ = app.drain_events();
    });
    assert_eq!(read(&view, cx, |app| { app.sessions.len() }), 2);
    assert!(
        !read(&view, cx, |app| { frame_text(app) }).contains("abc"),
        "Ctrl+D 应只删一个字符"
    );
}

#[gpui::test]
fn scrollbar_jump_scrolls_to_the_requested_offset(cx: &mut TestAppContext) {
    let (view, cx) = new_pty_session(cx);
    write_session(&view, cx, b"seq 1 200\n");
    assert!(
        poll_until(&view, cx, |app| { frame_text(app).contains("200") }),
        "seq 输出 15 秒内未到尾部"
    );
    let id = read(&view, cx, |app| app.active_pane);
    eprintln!(
        "DEBUG before scroll_to_top: rows={} history={:?} offset={:?}",
        read(&view, cx, |app| app.session().frame.rows.len()),
        read(&view, cx, |app| app.session().history_size()),
        read(&view, cx, |app| app.session().display_offset()),
    );
    view.update(cx, |app, _| app.scroll_to_top());
    eprintln!(
        "DEBUG after scroll_to_top: history={:?} offset={:?}",
        read(&view, cx, |app| app.session().history_size()),
        read(&view, cx, |app| app.session().display_offset()),
    );
    let offset = read(&view, cx, |app| app.session().display_offset());
    assert!(
        matches!(offset, Some(offset) if offset > 0),
        "滚到顶部后偏移应大于 0"
    );
    view.update(cx, |app, _| app.scroll_to_offset(id, 0));
    assert_eq!(
        read(&view, cx, |app| app.session().display_offset()),
        Some(0)
    );
    view.update(cx, |app, _| app.scroll_to_offset(id, 5));
    assert_eq!(
        read(&view, cx, |app| app.session().display_offset()),
        Some(5)
    );
}
