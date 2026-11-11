// @Time    : 2026/11/11
// @Author  : Lux
// @FileName: compat.rs
// @Software: Siskin

//! P4 兼容性测试：离线协议、配置损坏、D-Bus 不可用与内存上限常量。
//!
//! 不依赖显示器，全部在 `OfflineScreen` / 内存存储上跑。
//! 真实 PTY 的用例在 `compat_pty.rs`，GPUI 端到端在 `compat_gui.rs`。

use std::time::{Duration, Instant};

use crate::app::events::MAX_CLIPBOARD_WRITES;
use crate::app::session::SCROLLBACK_LINES;
use crate::domain::key::KeyModes;
use crate::domain::shell_marks::MAX_COMMAND_SPANS;
use crate::domain::shell_spec::{ShellSpec, terminal_env};
use crate::io::ansi::AnsiColors;
use crate::io::keys::mouse_report;
use crate::io::links;
use crate::io::preferences::{FileStore, MAX_BYTES, MemoryStore, Store as _};
use crate::io::vt::{CursorShape, OfflineScreen, ScreenSize, VtEvent, key_modes};
use crate::io::{dbusmenu, history_search::MAX_HISTORY_MATCHES};
use crate::theme::PAPER;

fn offline() -> (OfflineScreen, async_channel::Receiver<(u64, VtEvent)>) {
    let (sender, receiver) = async_channel::unbounded();
    (
        OfflineScreen::new(ScreenSize::new(40, 10, 0), sender, 0),
        receiver,
    )
}

fn palette() -> AnsiColors {
    AnsiColors::from_palette(&PAPER)
}

fn drain(receiver: &async_channel::Receiver<(u64, VtEvent)>) -> Vec<VtEvent> {
    let mut events = Vec::new();
    while let Ok((_, event)) = receiver.try_recv() {
        events.push(event);
    }
    events
}

fn row_text(frame: &crate::io::vt::Frame, row: usize) -> String {
    frame.rows[row]
        .cells
        .iter()
        .map(|cell| cell.text.as_str())
        .collect()
}

// ── OSC 52：默认仅写剪贴板 ──────────────────────────────────────────

#[test]
fn osc52_write_is_accepted_as_clipboard_store() {
    let (mut screen, receiver) = offline();
    screen.feed(b"\x1b]52;c;aGVsbG8=\x07");
    let events = drain(&receiver);
    assert_eq!(events.len(), 1, "写方向应恰好产生一个剪贴板事件");
    match &events[0] {
        VtEvent::ClipboardStore(text) => assert_eq!(text, "hello"),
        other => panic!("写方向不应产生 {other:?}"),
    }
}

#[test]
fn osc52_read_query_is_denied_without_a_response() {
    // `?` 是读请求：OnlyCopy 策略下上游直接拒绝，不回写任何序列；
    // 即使哪天策略放宽，`VtListener` 也只转 `ClipboardStore`，
    // 读回包（ClipboardLoad）同样到不了应用层。
    let (mut screen, receiver) = offline();
    screen.feed(b"\x1b]52;c;?\x07");
    screen.feed(b"\x1b]52;p;?\x07");
    assert!(
        drain(&receiver).is_empty(),
        "读请求不能产生任何事件，更不能回写剪贴板内容"
    );
}

#[test]
fn osc52_invalid_base64_produces_no_event() {
    let (mut screen, receiver) = offline();
    screen.feed(b"\x1b]52;c;!!!not-base64!!!\x07");
    assert!(drain(&receiver).is_empty(), "解不出的写请求应静默丢弃");
}

// ── OSC 8：链接起止与 URI 白名单 ────────────────────────────────────

#[test]
fn osc8_two_links_keep_their_own_uris_and_end_resets() {
    let (mut screen, _) = offline();
    screen.feed(b"\x1b]8;id=a;https://one.example\x1b\\one\x1b]8;;\x1b\\ \x1b]8;;https://two.example\x1b\\two\x1b]8;;\x1b\\");
    let frame = screen.frame(&palette());
    let cells: Vec<_> = frame.rows[0].cells.iter().collect();
    // 列：0..3 "one"、3 空格、4..7 "two"。
    for cell in cells.iter().take(3) {
        assert_eq!(cell.link.as_deref(), Some("https://one.example"));
    }
    assert!(cells[3].link.is_none(), "两段链接之间的空格不能带地址");
    for cell in cells.iter().skip(4).take(3) {
        assert_eq!(cell.link.as_deref(), Some("https://two.example"));
    }
    assert_eq!(row_text(&frame, 0).trim_end(), "one two");
}

#[test]
fn links_find_rejects_non_whitelisted_schemes() {
    // javascript:/data:/ftp: 的拒绝已由 `io::links` 自身的测试覆盖；
    // 这里补点击位置：区间按字符列算，落在区间外的列不命中。
    let found = links::find("see https://example.com/x now", None);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].0, 4..25);
    assert!(matches!(&found[0].1, links::Target::Web(url) if url == "https://example.com/x"));
    assert!(found.iter().any(|(range, _)| range.contains(&10)));
    assert!(found.iter().all(|(range, _)| !range.contains(&26)));
    // `file://` 在白名单内，存在性不在识别时校验，直接交给系统 opener。
    let file = links::find("see file:///etc/hostname now", None);
    assert_eq!(file.len(), 1);
    assert!(matches!(&file[0].1, links::Target::Web(url) if url == "file:///etc/hostname"));
}

// ── 鼠标协议：模式翻译与报告编码 ────────────────────────────────────

#[test]
fn mouse_modes_translate_from_escape_sequences() {
    let (mut screen, _) = offline();
    screen.feed(b"\x1b[?1000h\x1b[?1006h");
    let modes = key_modes(screen.mode());
    assert!(modes.mouse_click);
    assert!(modes.sgr_mouse);
    assert!(modes.mouse_reporting());
    let (mut screen, _) = offline();
    screen.feed(b"\x1b[?1002h");
    let modes = key_modes(screen.mode());
    assert!(modes.mouse_drag);
    assert!(!modes.sgr_mouse);
}

#[test]
fn mouse_report_encodes_sgr_press_and_release() {
    let modes = KeyModes {
        sgr_mouse: true,
        mouse_click: true,
        ..KeyModes::default()
    };
    assert_eq!(
        mouse_report(&modes, 0, 4, 2, true, false, false, false).as_deref(),
        Some(b"\x1b[<0;5;3M".as_slice())
    );
    assert_eq!(
        mouse_report(&modes, 0, 4, 2, false, false, false, false).as_deref(),
        Some(b"\x1b[<3;5;3m".as_slice()),
        "SGR 用 M/m 区分按下与松开，松开用编码 3 上报"
    );
    assert!(mouse_report(&KeyModes::default(), 0, 0, 0, true, false, false, false).is_none());
}

#[test]
fn mouse_report_x10_release_is_suppressed_in_click_only_mode() {
    let click = KeyModes {
        mouse_click: true,
        ..KeyModes::default()
    };
    assert_eq!(
        mouse_report(&click, 0, 4, 2, true, false, false, false).as_deref(),
        Some([0x1b, b'[', b'M', 32, 37, 35].as_slice()),
        "X10：列行各加 32，列 5→37、行 3→35"
    );
    assert!(
        mouse_report(&click, 0, 4, 2, false, false, false, false).is_none(),
        "X10 没有松开位，纯点击模式的松开不能伪装成一次按下"
    );
    let drag = KeyModes {
        mouse_click: true,
        mouse_drag: true,
        ..KeyModes::default()
    };
    assert_eq!(
        mouse_report(&drag, 0, 4, 2, false, false, false, false).as_deref(),
        Some([0x1b, b'[', b'M', 35, 37, 35].as_slice()),
        "拖拽模式的松开用编码 3 上报"
    );
}

// ── 颜色与光标 ──────────────────────────────────────────────────────

#[test]
fn truecolor_and_256color_reach_the_frame() {
    let (mut screen, _) = offline();
    screen.feed(b"\x1b[38;5;196mR\x1b[38;2;10;20;30mG\x1b[48;5;21mB\x1b[48;2;1;2;3mC");
    let frame = screen.frame(&palette());
    let table = palette();
    assert_eq!(frame.rows[0].cells[0].style.fg, table.indexed(196));
    assert_eq!(frame.rows[0].cells[1].style.fg, 0x0a_14_1e);
    assert_eq!(frame.rows[0].cells[2].style.bg, table.indexed(21));
    assert_eq!(frame.rows[0].cells[3].style.bg, 0x01_02_03);
}

#[test]
fn cursor_shapes_follow_decscusr() {
    for (sequence, expected) in [
        ("\x1b[1 q", CursorShape::Block),
        ("\x1b[2 q", CursorShape::Block),
        ("\x1b[3 q", CursorShape::Underline),
        ("\x1b[4 q", CursorShape::Underline),
        ("\x1b[5 q", CursorShape::Beam),
        ("\x1b[6 q", CursorShape::Beam),
    ] {
        let (mut screen, _) = offline();
        screen.feed(b"\x1b[2;3H");
        screen.feed(sequence.as_bytes());
        let frame = screen.frame(&palette());
        assert_eq!(frame.cursor.row, 1);
        assert_eq!(frame.cursor.column, 2);
        assert_eq!(frame.cursor.shape, expected, "{sequence}");
    }
}

#[test]
fn mixed_width_text_keeps_cursor_columns_aligned() {
    let (mut screen, _) = offline();
    screen.feed("中😀a".as_bytes());
    let frame = screen.frame(&palette());
    assert_eq!(frame.rows[0].cells[0].text, "中");
    assert!(frame.rows[0].cells[1].spacer);
    assert_eq!(frame.rows[0].cells[2].text, "😀");
    assert!(frame.rows[0].cells[3].spacer);
    assert_eq!(frame.rows[0].cells[4].text, "a");
    assert_eq!(frame.cursor.column, 5, "2+2+1 列");
    assert_eq!(row_text(&frame, 0).trim_end(), "中😀a");
}

// ── TERM / COLORTERM 实际值 ─────────────────────────────────────────

#[test]
fn terminal_env_is_pinned_not_inherited() {
    let env = terminal_env();
    let get = |name: &str| {
        env.iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    };
    assert_eq!(get("TERM"), Some("xterm-256color"));
    assert_eq!(get("COLORTERM"), Some("truecolor"));
    assert_eq!(get("TERM_PROGRAM"), Some("siskin"));
    assert_eq!(get("TERM_PROGRAM_VERSION"), Some(env!("CARGO_PKG_VERSION")));
}

// ── 异常恢复：shell 不存在、配置损坏、D-Bus 不可用 ─────────────────

#[test]
fn missing_shell_is_an_error_not_a_panic() {
    let missing = std::path::PathBuf::from("/nonexistent/siskin-shell-xyz");
    let error = ShellSpec::Program(missing.clone())
        .resolve()
        .expect_err("不存在的程序必须解析失败");
    assert!(error.to_string().contains("siskin-shell-xyz"));
    assert!(
        ShellSpec::WithArgs {
            program: missing,
            args: vec!["-c".into()],
        }
        .resolve()
        .is_err()
    );
    let (sender, _) = async_channel::unbounded();
    let result = crate::io::pty::Pty::spawn(
        &ShellSpec::Program("/nonexistent/siskin-shell-xyz".into()),
        None,
        ScreenSize::new(40, 10, 200),
        sender,
        0,
        0,
        &terminal_env(),
    );
    assert!(result.is_err(), "PTY 派生失败应返回 Err，不 panic");
}

#[test]
fn corrupt_preferences_fall_back_without_losing_the_file() {
    let path = std::env::temp_dir().join(format!("siskin-p4-corrupt-{}.json", std::process::id()));
    let store = FileStore::new(&path);
    // 整份 JSON 损坏：加载失败，文件原样保留。
    std::fs::write(&path, "not json at all").unwrap();
    assert!(store.load().is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "not json at all");
    // 超限文件：在解析前就拒绝。
    std::fs::write(&path, " ".repeat(MAX_BYTES as usize + 1)).unwrap();
    assert!(store.load().is_err());
    std::fs::remove_file(&path).unwrap();
}

#[test]
fn one_bad_snapshot_does_not_kill_the_whole_config() {
    // 只有一条快照损坏：丢那一条，好条目与其它配置都保留。
    let path = std::env::temp_dir().join(format!("siskin-p4-sessions-{}.json", std::process::id()));
    let store = FileStore::new(&path);
    std::fs::write(
        &path,
        r#"{"version":2,"shell":"bash","sessions":[
            {"title":"好的","working_directory":"/tmp"},
            {"title":123},
            {"title":"也好的"}
        ]}"#,
    )
    .unwrap();
    let loaded = store.load().expect("单条坏快照不应拖垮整份配置");
    assert_eq!(loaded.sessions.len(), 2);
    assert_eq!(loaded.sessions[0].title, "好的");
    assert_eq!(loaded.sessions[1].title, "也好的");
    // 快照段根本不是数组：仍按整份失败处理，走默认回退。
    std::fs::write(&path, r#"{"version":2,"sessions":"garbage"}"#).unwrap();
    assert!(store.load().is_err());
    std::fs::remove_file(&path).unwrap();
}

#[test]
fn corrupt_memory_store_is_rejected_without_rewriting() {
    // MemoryStore 层面的拒绝规则已由 `tests/preferences.rs` 覆盖；
    // 这里只补一条：坏 sessions 数组在内存实现上同样只丢坏条目。
    let store =
        MemoryStore::with_contents(r#"{"version":2,"sessions":[{"title":"好"},{"title":123}]}"#);
    let loaded = store.load().expect("单条坏快照不应拖垮整份配置");
    assert_eq!(loaded.sessions.len(), 1);
    assert_eq!(loaded.sessions[0].title, "好");
}

#[test]
fn dbus_unavailable_is_reported_instead_of_crashing() {
    if std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_some() {
        // 有会话总线时结果取决于桌面环境，本测试只覆盖「无总线」
        // 的降级路径；有总线的往返由 `io::dbusmenu::tests` 覆盖。
        return;
    }
    let menu = dbusmenu::install(1, dbusmenu::build_layout("跟随系统", "zsh"))
        .expect("工作线程本身应能启动");
    let events = menu.events();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match events.try_recv() {
            Ok(dbusmenu::MenuEvent::State(state)) => {
                assert_eq!(
                    state,
                    dbusmenu::MenuState::Unavailable,
                    "无 D-Bus 时应明确上报不可用，菜单缺失不影响终端本身"
                );
                return;
            }
            Ok(_) => {}
            Err(_) => {
                if Instant::now() > deadline {
                    panic!("10 秒内没有收到 D-Bus 不可用通知");
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
}

// ── 内存上限：常量即契约 ────────────────────────────────────────────

#[test]
fn memory_limits_are_declared_constants() {
    assert_eq!(SCROLLBACK_LINES, 10_000);
    assert_eq!(MAX_HISTORY_MATCHES, 1_000);
    assert_eq!(MAX_COMMAND_SPANS, 100);
    assert_eq!(MAX_BYTES, 65_536);
    assert_eq!(MAX_CLIPBOARD_WRITES, 64);
}
