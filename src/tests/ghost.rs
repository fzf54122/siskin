// @Time    : 2026/11/11
// @Author  : fzf
// @FileName: ghost.rs
// @Software: Siskin

//! 行内自动补全（ghost text）按键测试：真实 PTY 上 Tab 接受整条、
//! Right 接受一个词、Esc 取消、与命令面板的互不干扰。
//! `C` 捕获进建议来源的测试在 `tests/pty_osc133.rs`（标记侧）。
//!
//! `poll_until` / `new_pty_session` / `write_session` / `frame_text`
//! 与 `tests/compat_gui.rs` 的 harness 同形（那个文件不动，
//! 这里复制一份只为 ghost 用）；`cat` 用来接管输入——它只回显，
//! 不做行编辑，提示符锚点由测试直接按帧光标注入。

use std::time::{Duration, Instant};

use gpui::{Entity, TestAppContext, VisualTestContext};

use super::views::{click, read, window};
use crate::app::{Handled, SiskinApp};
use crate::domain::shell_marks::{Osc133Mark, ShellMark};

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

fn send(app: &mut SiskinApp, text: &str) -> Option<Handled> {
    let keystroke = gpui::Keystroke::parse(text).expect("测试里的按键写法必须合法");
    app.handle_key(&keystroke)
}

/// 光标所在行的文本（去尾空白）：当前输入行就是它。
fn cursor_line_text(app: &SiskinApp) -> String {
    let frame = &app.session().frame;
    frame
        .rows
        .get(frame.cursor.row)
        .map(|row| {
            row.cells
                .iter()
                .map(|cell| cell.text.as_str())
                .collect::<String>()
        })
        .unwrap_or_default()
        .trim_end()
        .to_string()
}

/// 在当前光标位置注入一个 OSC 133 标记：行列取自帧光标，
/// 与读线程锚定的一致（`B` 的列就是提示符末尾）。
fn inject_mark(view: &Entity<SiskinApp>, cx: &mut VisualTestContext, kind: Osc133Mark) {
    view.update(cx, |app, _| {
        let session = app.session_mut();
        let line = session.history_size().unwrap_or(0) as i32 + session.frame.cursor.row as i32;
        let column = session.frame.cursor.column as i32;
        session.apply_shell_mark(ShellMark::new(line, column, kind));
    });
}

/// 在 `cat` 里跑一条命令并让 ghost 记下来：`A` → `B` 锚点 → 输入 → 回车 → `C` 捕获。
/// `probe` / `want_suffix` 用来断言这条命令确实进了建议来源。
fn record_command(
    view: &Entity<SiskinApp>,
    cx: &mut VisualTestContext,
    command: &str,
    probe: &str,
    want_suffix: &str,
) {
    inject_mark(view, cx, Osc133Mark::PromptStart);
    inject_mark(view, cx, Osc133Mark::CommandInput);
    write_session(view, cx, format!("{command}\n").as_bytes());
    assert!(
        poll_until(view, cx, |app| frame_text(app).contains(command)),
        "命令回显 15 秒内未出现：{command}"
    );
    inject_mark(view, cx, Osc133Mark::PreExec);
    assert!(
        poll_until(view, cx, |app| app
            .session()
            .ghost
            .history
            .suggest(probe)
            .as_deref()
            == Some(want_suffix)),
        "命令未进建议来源：{command}"
    );
}

/// 带真实 PTY 的 ghost 会话：默认 shell 里跑 `cat`，
/// 建议来源里已有两条命令，当前行是空行，`B` 锚点已注入。
fn ghost_ready(cx: &mut TestAppContext) -> (Entity<SiskinApp>, &mut VisualTestContext) {
    let (view, cx) = new_pty_session(cx);
    assert!(
        poll_until(&view, cx, |app| !frame_text(app).trim().is_empty()),
        "shell 15 秒内未输出提示符"
    );
    write_session(&view, cx, b"cat\n");
    assert!(
        poll_until(&view, cx, |app| {
            frame_text(app).contains("cat") && app.session().frame.cursor.column == 0
        }),
        "cat 15 秒内未接管输入"
    );
    record_command(&view, cx, "git status", "git s", "tatus");
    record_command(&view, cx, "git commit -m \"x\"", "git c", "ommit -m \"x\"");
    inject_mark(&view, cx, Osc133Mark::PromptStart);
    inject_mark(&view, cx, Osc133Mark::CommandInput);
    (view, cx)
}

#[gpui::test]
fn tab_accepts_the_whole_suggestion(cx: &mut TestAppContext) {
    let (view, cx) = ghost_ready(cx);
    write_session(&view, cx, b"git ");
    assert!(
        poll_until(&view, cx, |app| app.session().ghost_suffix().as_deref()
            == Some("commit -m \"x\"")),
        "输入 git 后应联想 commit -m \"x\""
    );
    view.update(cx, |app, _| {
        assert!(matches!(send(app, "tab"), Some(Handled::Redraw)));
    });
    assert!(
        poll_until(&view, cx, |app| cursor_line_text(app)
            == "git commit -m \"x\""),
        "Tab 应把整条后缀写进 PTY，帧内容：\n{}",
        read(&view, cx, frame_text)
    );
    assert!(
        poll_until(&view, cx, |app| app.session().ghost_suffix().is_none()),
        "接受后输入已完整，不应再有 ghost"
    );
}

#[gpui::test]
fn right_arrow_accepts_one_word_at_a_time(cx: &mut TestAppContext) {
    let (view, cx) = ghost_ready(cx);
    write_session(&view, cx, b"git ");
    assert!(
        poll_until(&view, cx, |app| app.session().ghost_suffix().is_some()),
        "输入 git 后应有联想"
    );
    view.update(cx, |app, _| {
        assert!(matches!(send(app, "right"), Some(Handled::Redraw)));
    });
    assert!(
        poll_until(&view, cx, |app| cursor_line_text(app) == "git commit"),
        "右方向键应只接受一个词，帧内容：\n{}",
        read(&view, cx, frame_text)
    );
    view.update(cx, |app, _| {
        assert!(matches!(send(app, "right"), Some(Handled::Redraw)));
    });
    assert!(
        poll_until(&view, cx, |app| cursor_line_text(app) == "git commit -m"),
        "第二次右方向键应接受下一个词，帧内容：\n{}",
        read(&view, cx, frame_text)
    );
    view.update(cx, |app, _| {
        assert!(matches!(send(app, "tab"), Some(Handled::Redraw)));
    });
    assert!(
        poll_until(&view, cx, |app| cursor_line_text(app)
            == "git commit -m \"x\""),
        "Tab 应补全剩余部分，帧内容：\n{}",
        read(&view, cx, frame_text)
    );
}

#[gpui::test]
fn escape_cancels_the_suggestion_without_sending_bytes(cx: &mut TestAppContext) {
    let (view, cx) = ghost_ready(cx);
    write_session(&view, cx, b"git ");
    assert!(
        poll_until(&view, cx, |app| app.session().ghost_suffix().is_some()),
        "输入 git 后应有联想"
    );
    view.update(cx, |app, _| {
        assert!(matches!(send(app, "escape"), Some(Handled::Redraw)));
    });
    assert!(
        poll_until(&view, cx, |app| app.session().ghost_suffix().is_none()),
        "Esc 应取消联想"
    );
    assert_eq!(
        read(&view, cx, cursor_line_text),
        "git",
        "Esc 不应向 shell 发字节"
    );
    // 取消后 Tab 不再补全：它原样进 PTY（cat 回显一个制表位），
    // 光标移到制表位，行内容去尾空白后仍是 `git`。
    view.update(cx, |app, _| {
        assert!(matches!(send(app, "tab"), Some(Handled::Redraw)));
    });
    assert!(
        poll_until(&view, cx, |app| app.session().frame.cursor.column >= 8),
        "Tab 应原样进 PTY，光标应移到制表位"
    );
    assert_eq!(
        read(&view, cx, cursor_line_text),
        "git",
        "Tab 不应补全被取消的建议"
    );
    assert_eq!(
        read(&view, cx, |app| app.session().ghost_suffix()),
        None,
        "取消后仍不应有联想"
    );
}

#[gpui::test]
fn tab_does_not_accept_a_suggestion_while_the_palette_is_open(cx: &mut TestAppContext) {
    let (view, cx) = ghost_ready(cx);
    write_session(&view, cx, b"git ");
    assert!(
        poll_until(&view, cx, |app| app.session().ghost_suffix().is_some()),
        "输入 git 后应有联想"
    );
    view.update(cx, |app, _| {
        assert!(matches!(send(app, "ctrl-k"), Some(Handled::Redraw)));
        assert!(app.palette_open());
    });
    view.update(cx, |app, _| {
        send(app, "tab");
        assert!(app.palette_open(), "Tab 不应关掉命令面板");
    });
    assert_eq!(
        read(&view, cx, cursor_line_text),
        "git",
        "面板打开时 Tab 不应接受补全"
    );
    view.update(cx, |app, _| {
        send(app, "escape");
        assert!(!app.palette_open());
    });
}
