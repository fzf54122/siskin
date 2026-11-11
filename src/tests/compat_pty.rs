// @Time    : 2026/11/11
// @Author  : Lux
// @FileName: compat_pty.rs
// @Software: Siskin

//! P4 兼容性测试：真实 PTY。TERM/COLORTERM、resize、`stty size`、
//! 异常退出、zsh/tmux、OSC 52 真实派生路径与 `seq 1M` 内存上限。
//! 不依赖显示器；vim/htop/fish 本机未装，见 `docs/compat-matrix.md`。

use std::time::{Duration, Instant};

use alacritty_terminal::term::TermMode;

use crate::app::session::SCROLLBACK_LINES;
use crate::domain::session::SessionSpec;
use crate::domain::shell_spec::terminal_env;
use crate::io::ansi::AnsiColors;
use crate::io::pty::Pty;
use crate::io::vt::{ScreenSize, VtEvent};
use crate::theme::PAPER;

use super::pty::wait_for_output;

fn spawn_with_env(
    spec: SessionSpec,
    env: &[(String, String)],
) -> (Pty, async_channel::Receiver<(u64, VtEvent)>) {
    let (sender, receiver) = async_channel::unbounded();
    let pty = Pty::spawn(
        &spec.shell,
        spec.working_directory,
        ScreenSize::new(40, 10, 10_000),
        sender,
        0,
        0,
        env,
    )
    .expect("测试环境必须能派生 PTY");
    (pty, receiver)
}

fn command(program: &str, script: &str) -> SessionSpec {
    SessionSpec::command(
        "test",
        std::path::PathBuf::from(program),
        vec!["-c".into(), script.into()],
    )
}

/// 轮询到 `ChildExit` 或超时。`seq` 这类长输出测试与其它 PTY 测试
/// 并行抢 1.5 核，15 秒不够用，这里给 90 秒（与 `pty_drain.rs` 一致）。
fn wait_child_exit(
    events: &async_channel::Receiver<(u64, VtEvent)>,
    needle: &str,
    timeout: Duration,
) -> Option<i32> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Ok((_, VtEvent::ChildExit(code))) = events.try_recv() {
            return code;
        }
        if Instant::now() > deadline {
            panic!("{needle}：超时未收到 ChildExit 事件");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn wait_for_output_long(pty: &Pty, needle: &str, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
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

#[test]
fn terminal_env_is_what_programs_see() {
    let spec = command(
        "/bin/sh",
        "printf '%s|%s|%s|%s' \"$TERM\" \"$COLORTERM\" \"$TERM_PROGRAM\" \"$TERM_PROGRAM_VERSION\"",
    );
    let (pty, _) = spawn_with_env(spec, &terminal_env());
    let expected = format!(
        "xterm-256color|truecolor|siskin|{}",
        env!("CARGO_PKG_VERSION")
    );
    assert!(
        wait_for_output(&pty, &expected),
        "子进程看到的环境应为 {expected}"
    );
}

#[test]
fn resize_reports_new_size_and_output_keeps_flowing() {
    let spec = command(
        "/bin/sh",
        "printf 'ready\\n'; read wake; stty size; echo after; sleep 0.5",
    );
    let (mut pty, _) = spawn_with_env(spec, &terminal_env());
    assert!(wait_for_output(&pty, "ready"), "子进程未进入就绪状态");
    pty.resize(ScreenSize::new(60, 15, 200));
    assert_eq!(pty.size().columns, 60);
    pty.write(b"\r");
    assert!(wait_for_output(&pty, "15 60"), "stty size 应报告新尺寸");
    assert_eq!(
        pty.frame(&AnsiColors::from_palette(&PAPER)).rows.len(),
        15,
        "网格行数应跟随新尺寸"
    );
    assert!(wait_for_output(&pty, "after"), "resize 后输出应继续流动");
}

#[test]
fn signal_kill_is_reported_as_no_code_not_zero() {
    let spec = command("/bin/sh", "kill -9 $$");
    let (_pty, events) = spawn_with_env(spec, &terminal_env());
    let code = wait_child_exit(&events, "kill -9 $$", Duration::from_secs(15));
    assert_eq!(
        code, None,
        "信号杀死没有退出码，必须如实报 None，不能谎报 0"
    );
}

#[test]
fn exit_137_is_reported_verbatim() {
    let spec = command("/bin/sh", "exit 137");
    let (_pty, events) = spawn_with_env(spec, &terminal_env());
    let code = wait_child_exit(&events, "exit 137", Duration::from_secs(15));
    assert_eq!(code, Some(137));
}

#[test]
fn zsh_runs_under_the_pty() {
    let spec = command("/usr/bin/zsh", "echo zsh-ok-$ZSH_VERSION");
    let (pty, _) = spawn_with_env(spec, &terminal_env());
    assert!(wait_for_output(&pty, "zsh-ok-"), "zsh 未在 PTY 内输出");
}

#[test]
fn tmux_starts_and_enables_sgr_mouse() {
    let pid = std::process::id();
    let socket = format!("siskin-p4-{pid}");
    let conf = std::env::temp_dir().join(format!("siskin-p4-tmux-{pid}.conf"));
    std::fs::write(&conf, "set -g mouse on\n").unwrap();
    let spec = SessionSpec::command(
        "test",
        std::path::PathBuf::from("/usr/bin/tmux"),
        vec![
            "-L".into(),
            socket.clone(),
            "-f".into(),
            conf.to_string_lossy().into_owned(),
            "new-session".into(),
            "-s".into(),
            "p4".into(),
        ],
    );
    let (pty, _) = spawn_with_env(spec, &terminal_env());
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut sgr = false;
    while Instant::now() < deadline {
        if pty.mode().contains(TermMode::SGR_MOUSE) {
            sgr = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let _ = std::process::Command::new("/usr/bin/tmux")
        .args(["-L", &socket, "kill-server"])
        .output();
    let _ = std::fs::remove_file(&conf);
    assert!(sgr, "tmux 开 mouse 后应启用 SGR 鼠标报告（1006）");
}

#[test]
fn osc52_write_reaches_the_app_through_a_real_pty() {
    // 子程序自己输出序列：写进主端走行规程回显时，ECHOCTL 会把
    // ESC 变成可见的 `^[`，解析器永远看不到真正的转义序列。
    let spec = command("/bin/sh", "printf '\\033]52;c;aGVsbG8=\\033\\\\'; sleep 30");
    let (_pty, events) = spawn_with_env(spec, &terminal_env());
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if let Ok((_, VtEvent::ClipboardStore(text))) = events.try_recv() {
            assert_eq!(text, "hello");
            return;
        }
        if Instant::now() > deadline {
            panic!("真实 PTY 上 OSC 52 写请求未到达应用层");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn osc52_read_query_gets_no_answer_through_a_real_pty() {
    let spec = command("/bin/sh", "printf '\\033]52;c;?\\033\\\\'; sleep 30");
    let (_pty, events) = spawn_with_env(spec, &terminal_env());
    // 500ms 内收不到 ClipboardStore / PtyWrite 即算拒绝成立：
    // Wakeup 只是「有输出到达」，读请求本身不会触发回写。
    let deadline = Instant::now() + Duration::from_millis(500);
    while Instant::now() < deadline {
        if let Ok((_, event)) = events.try_recv() {
            match event {
                VtEvent::ClipboardStore(_) | VtEvent::PtyWrite(_) => {
                    panic!("读请求不应产生 {event:?}");
                }
                _ => {}
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn vmrss_kib() -> u64 {
    let Ok(status) = std::fs::read_to_string("/proc/self/status") else {
        return 0;
    };
    for line in status.lines() {
        if let Some(value) = line.strip_prefix("VmRSS:") {
            return value
                .trim()
                .trim_end_matches("kB")
                .trim()
                .parse()
                .unwrap_or(0);
        }
    }
    0
}

#[test]
fn seq_1m_history_is_capped_and_memory_growth_is_bounded() {
    // 实测（2026-11-11，本机 1.5 核 VM）：
    // 单跑：RSS 增长 17,720 KiB；全套测试并行时 32,584 KiB
    // （同进程里的其它 PTY 测试会分摊进来，这是噪声不是泄漏）。
    // 上界取全套实测的 2 倍（65,168 KiB）取整为 64 MiB。
    // 若本测试持续逼近上界，说明 scrollback 之外的结构在随
    // 输出增长，需要复查；并行噪声本身不是失败理由。
    let before = vmrss_kib();
    let spec = command("/bin/sh", "seq 1 1000000; sleep 10");
    let (pty, _) = spawn_with_env(spec, &terminal_env());
    assert!(
        wait_for_output_long(&pty, "1000000", Duration::from_secs(90)),
        "seq 1M 未跑完"
    );
    let history = pty.history_size();
    let growth_kib = vmrss_kib().saturating_sub(before);
    eprintln!("siskin-p4-memory: history={history} rss_before={before}KiB growth={growth_kib}KiB");
    assert_eq!(history, SCROLLBACK_LINES, "历史必须截断到 10,000 行");
    assert!(
        growth_kib < 64 * 1024,
        "seq 1M 后 RSS 增长 {growth_kib} KiB，超过 64 MiB 上界"
    );
}
