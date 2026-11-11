// @Time    : 2026/11/5 18:55
// @Author  : fzf
// @FileName: pty.rs
// @Software: Siskin

//! 真实 PTY 集成测试：起 `sh -c` 跑命令，断言读到的网格内容与退出码。
//! 不依赖显示器，CI 里也能跑。

use std::time::{Duration, Instant};

use crate::domain::session::SessionSpec;
use crate::domain::shell_spec::ShellKind;
use crate::io::ansi::AnsiColors;
use crate::io::pty::Pty;
use crate::io::vt::{ScreenSize, VtEvent};
use crate::theme::PAPER;

/// 轮询到网格里出现 `needle` 或超时。PTY 输出是异步的，固定 sleep
/// 在慢机器上会随机失败。
pub(super) fn wait_for_output(pty: &Pty, needle: &str) -> bool {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let frame = pty.frame(&AnsiColors::from_palette(&PAPER));
        let text: String = frame
            .rows
            .iter()
            .flat_map(|row| row.cells.iter())
            .map(|cell| cell.text.as_str())
            .collect();
        if text.contains(needle) {
            return true;
        }
        if Instant::now() > deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

pub(super) fn spawn(spec: SessionSpec) -> (Pty, async_channel::Receiver<(u64, VtEvent)>) {
    let (sender, receiver) = async_channel::unbounded();
    let pty = Pty::spawn(
        &spec.shell,
        spec.working_directory,
        ScreenSize::new(40, 10, 200),
        sender,
        0,
        0,
        &[("TERM".into(), "xterm-256color".into())],
    )
    .expect("测试环境必须能派生 PTY");
    (pty, receiver)
}

#[test]
fn command_output_reaches_the_grid() {
    // 打印后短暂停留：子进程与「最后一个字节」同时退出时，上游事件循环
    // 可能在 drain 之前就看到 SIGCHLD，属于 alacritty_terminal 的边界行为。
    // 退出路径本身由 `exit_status_is_reported_through_events` 覆盖。
    let spec = SessionSpec::command(
        "test",
        std::path::PathBuf::from("/bin/sh"),
        vec!["-c".into(), "printf 'hello-pty'; sleep 0.05".into()],
    );
    let (pty, _events) = spawn(spec);
    assert!(wait_for_output(&pty, "hello-pty"), "未读到命令输出");
}

#[test]
fn exit_status_is_reported_through_events() {
    let spec = SessionSpec::command(
        "test",
        std::path::PathBuf::from("/bin/sh"),
        vec!["-c".into(), "exit 3".into()],
    );
    let (_pty, events) = spawn(spec);
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if let Ok((_, VtEvent::ChildExit(code))) = events.try_recv() {
            assert_eq!(code, Some(3));
            return;
        }
        if Instant::now() > deadline {
            panic!("没有收到 ChildExit 事件");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn resize_updates_the_pty_window_size() {
    // 子进程先打印 ready，父进程看到之后才 resize 并写入。
    // 这样顺序是「子进程已阻塞在 read → 改尺寸 → 唤醒 → 读尺寸」，
    // 不受调度影响。
    let spec = SessionSpec::command(
        "test",
        std::path::PathBuf::from("/bin/sh"),
        vec![
            "-c".into(),
            "printf 'ready\\n'; read wake; stty size; sleep 0.2".into(),
        ],
    );
    let (mut pty, _events) = spawn(spec);
    assert!(wait_for_output(&pty, "ready"), "子进程未进入就绪状态");
    pty.resize(ScreenSize::new(100, 20, 200));
    assert_eq!(pty.size().columns, 100);
    pty.write(b"\r");
    assert!(wait_for_output(&pty, "20 100"), "stty size 应报告新尺寸");
}

#[test]
fn input_reaches_the_pty_line_discipline() {
    // 行规程会把输入回显到 master 端，这一步不依赖子进程何时执行 read，
    // 因此在并行负载下也是确定的；它验证的是「写入路径本身畅通」。
    let spec = SessionSpec::command(
        "test",
        std::path::PathBuf::from("/bin/sh"),
        vec!["-c".into(), "sleep 3".into()],
    );
    let (mut pty, _events) = spawn(spec);
    pty.write(b"ping-echo\r");
    assert!(wait_for_output(&pty, "ping-echo"), "输入没有到达 PTY");
}

#[test]
fn a_missing_shell_is_reported_before_spawning() {
    let spec = SessionSpec::command(
        "test",
        std::path::PathBuf::from("/nonexistent/shell"),
        Vec::new(),
    );
    let (sender, _receiver) = async_channel::unbounded();
    let result = Pty::spawn(
        &spec.shell,
        None,
        ScreenSize::new(20, 5, 0),
        sender,
        0,
        0,
        &[],
    );
    assert!(result.is_err());
}

/// 脏帧标记的契约。`drain_events` 每轮只刷新被标记的会话，
/// 否则多会话场景下，每轮都会为所有会话白构一次 Frame。
/// 这里直接验证「标记 → 跳过」的规则，不依赖异步时序。
#[test]
fn only_sessions_with_a_pty_are_marked_dirty() {
    use crate::app::{Session, Sessions};

    let mut set = Sessions::new(Session::starting(7, "输出会话", ShellKind::Sh));
    set.add(Session::starting(8, "空闲会话", ShellKind::Sh));
    for index in 0..set.len() {
        set.get_mut(index).expect("下标有效").mark_output();
    }
    // 没有 PTY 的会话不该被标记，否则每轮都会为它白构一次帧。
    assert!(!set.get(0).expect("下标有效").frame_dirty());
    assert!(!set.get(1).expect("下标有效").frame_dirty());
}

/// 派生后的会话：有输出就该置脏，构帧后清除。
/// 这条走真实 PTY，验证 `mark_output` 与 `refresh` 在真实会话上闭环。
#[test]
fn a_running_session_is_marked_and_cleared_on_refresh() {
    let spec = SessionSpec::command(
        "test",
        std::path::PathBuf::from("/bin/sh"),
        vec!["-c".into(), "sleep 2".into()],
    );
    let palette = AnsiColors::from_palette(&PAPER);
    let mut session = crate::app::Session::starting(4, "真实会话", ShellKind::Sh);
    let (sender, receiver) = async_channel::unbounded();
    session
        .spawn_spec(&spec, sender, 0)
        .expect("测试环境必须能派生 PTY");
    session.mark_output();
    assert!(session.frame_dirty(), "有 PTY 且收到输出就该置脏");
    session.refresh(&palette);
    assert!(!session.frame_dirty(), "构帧后必须清除，否则下一轮重复构帧");
    assert!(!session.frame.rows.is_empty(), "构帧后应有网格内容");
    drop(receiver);
}

/// 派生失败时不该置脏标记，否则每轮都会为一个没有 PTY 的会话构帧。
#[test]
fn a_failed_spawn_leaves_the_frame_clean() {
    let spec = SessionSpec::command(
        "test",
        std::path::PathBuf::from("/nonexistent/shell"),
        Vec::new(),
    );
    let mut session = crate::app::Session::starting(11, "失败会话", ShellKind::Sh);
    let (sender, _receiver) = async_channel::unbounded();
    assert!(session.spawn_spec(&spec, sender, 0).is_err());
    session.mark_output();
    assert!(!session.frame_dirty(), "派生失败的会话不该被标记为脏");
    session.refresh(&AnsiColors::from_palette(&PAPER));
    assert!(!session.frame_dirty());
}

pub(super) fn session_text(session: &crate::app::Session) -> String {
    session
        .frame
        .rows
        .iter()
        .flat_map(|row| row.cells.iter())
        .map(|cell| cell.text.as_str())
        .collect()
}

/// 多次取用同一快照不重复深拷贝：两次 `frame_snapshot` 指向同一份 `Arc`。
#[test]
fn repeated_snapshots_share_one_frame_without_a_deep_copy() {
    let session = crate::app::Session::starting(20, "快照会话", ShellKind::Sh);
    let first = session.frame_snapshot();
    let second = session.frame_snapshot();
    assert!(
        std::sync::Arc::ptr_eq(&first, &second),
        "两次取用必须是同一份 Arc，不能各拷一份 Frame"
    );
    assert!(
        std::sync::Arc::strong_count(&first) >= 3,
        "会话本身 + 两次取用应共享同一 Arc"
    );
}

/// 干净会话的 `refresh` 不替换快照：不脏就不构帧、不换指针。
#[test]
fn a_clean_refresh_keeps_the_same_snapshot() {
    let palette = AnsiColors::from_palette(&PAPER);
    let mut session = crate::app::Session::starting(22, "干净会话", ShellKind::Sh);
    let before = session.frame_snapshot();
    session.refresh(&palette);
    assert!(
        std::sync::Arc::ptr_eq(&before, &session.frame_snapshot()),
        "干净时 refresh 不该替换 Arc"
    );
    assert!(!session.frame_dirty());
}

/// 刷新替换 `Arc` 而不是原地改帧：旧快照仍是刷新前的内容，
/// 新快照才有新输出，渲染与搜索永远读不到半更新帧。
/// 走真实 PTY，验证 `mark_output` → `refresh` 的闭环。
#[test]
fn refresh_swaps_the_shared_snapshot_without_touching_the_old_one() {
    let spec = SessionSpec::command(
        "test",
        std::path::PathBuf::from("/bin/sh"),
        vec![
            "-c".into(),
            "printf 'one'; sleep 0.5; printf 'two'; sleep 2".into(),
        ],
    );
    let palette = AnsiColors::from_palette(&PAPER);
    let mut session = crate::app::Session::starting(21, "快照会话", ShellKind::Sh);
    let (sender, _receiver) = async_channel::unbounded();
    session
        .spawn_spec(&spec, sender, 0)
        .expect("测试环境必须能派生 PTY");
    let deadline = Instant::now() + Duration::from_secs(15);
    let old = loop {
        session.mark_output();
        session.refresh(&palette);
        if session_text(&session).contains("one") {
            break session.frame_snapshot();
        }
        if Instant::now() > deadline {
            panic!("未读到第一段输出");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let new = loop {
        session.mark_output();
        session.refresh(&palette);
        if session_text(&session).contains("two") {
            break session.frame_snapshot();
        }
        if Instant::now() > deadline {
            panic!("未读到第二段输出");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    assert!(
        !std::sync::Arc::ptr_eq(&old, &new),
        "刷新必须替换 Arc，不能原地改旧帧"
    );
    let old_text: String = old
        .rows
        .iter()
        .flat_map(|row| row.cells.iter())
        .map(|cell| cell.text.as_str())
        .collect();
    assert!(old_text.contains("one"), "旧快照应保留刷新前的内容");
    assert!(!old_text.contains("two"), "旧快照不能混进刷新后的输出");
    assert!(session_text(&session).contains("two"), "新快照应有新输出");
}
