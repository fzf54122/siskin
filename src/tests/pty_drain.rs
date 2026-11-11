// @Time    : 2026/11/11
// @Author  : fzf
// @FileName: pty_drain.rs
// @Software: Siskin

//! P2 事件合并的真实 PTY 压力测试：大输出、并发会话、唤醒洪泛。
//! 不依赖显示器，CI 里也能跑。复用 `pty.rs` 的 `spawn`。

use std::time::{Duration, Instant};

use crate::domain::session::SessionSpec;
use crate::io::ansi::AnsiColors;
use crate::io::pty::Pty;
use crate::io::vt::{ScreenSize, VtEvent};
use crate::theme::PAPER;

use super::pty::spawn;

/// 取 PTY 当前网格的全部可见文本（含滚动历史在帧内的部分）。
fn frame_text(pty: &Pty) -> String {
    pty.frame(&AnsiColors::from_palette(&PAPER))
        .rows
        .iter()
        .flat_map(|row| row.cells.iter())
        .map(|cell| cell.text.as_str())
        .collect()
}

/// 按给定间隔排空事件通道，直到看到子进程退出且队列排空。
/// 返回（轮次，事件数，退出码）。退出事件不丢失、不乱序是 P2 的验收项。
fn drain_until_exit(
    events: &async_channel::Receiver<(u64, VtEvent)>,
    interval: Duration,
) -> (u32, u32, Option<i32>) {
    let deadline = Instant::now() + Duration::from_secs(90);
    let mut rounds = 0u32;
    let mut total = 0u32;
    let mut exit: Option<Option<i32>> = None;
    loop {
        rounds += 1;
        let mut drained_this_round = 0u32;
        while let Ok((_, event)) = events.try_recv() {
            total += 1;
            drained_this_round += 1;
            if let VtEvent::ChildExit(code) = event {
                exit = Some(code);
            }
        }
        // 看到退出后再排一整轮空队列，确认没有事件滞留在通道里。
        if let Some(code) = exit
            && drained_this_round == 0
        {
            return (rounds, total, code);
        }
        if Instant::now() > deadline {
            panic!("90 秒内未等到子进程退出（已排 {total} 个事件）");
        }
        std::thread::sleep(interval);
    }
}

/// `seq 1 1000000` 的完整管线：PTY 读线程 → 事件通道 → 按间隔
/// 排空 → 网格。断言尾部数字在帧里、退出码是 0，即无丢字节、无卡死。
/// 用 `--nocapture` 跑能看到耗时/轮次/事件数，供 plan.md 记录窗口对比。
fn run_seq_1m(interval_ms: u64) -> (Duration, u32, u32) {
    let spec = SessionSpec::command(
        "test",
        std::path::PathBuf::from("/bin/sh"),
        vec!["-c".into(), "seq 1 1000000".into()],
    );
    let (pty, events) = spawn(spec);
    let started = Instant::now();
    let (rounds, total, code) = drain_until_exit(&events, Duration::from_millis(interval_ms));
    let elapsed = started.elapsed();
    assert_eq!(code, Some(0), "seq 应正常退出");
    assert!(
        frame_text(&pty).contains("1000000"),
        "最后一个数字必须落在帧里，否则丢了尾部字节"
    );
    eprintln!(
        "[p2-measure] interval={interval_ms}ms seq-1m elapsed={elapsed:?} rounds={rounds} events={total}"
    );
    (elapsed, rounds, total)
}

#[test]
fn seq_1m_without_lost_bytes_at_16ms() {
    run_seq_1m(16);
}

#[test]
fn seq_1m_without_lost_bytes_at_2ms() {
    run_seq_1m(2);
}

#[test]
fn three_sessions_output_in_parallel_without_loss() {
    let mut ptys = Vec::new();
    let mut receivers = Vec::new();
    for id in 0..3u64 {
        let spec = SessionSpec::command(
            "test",
            std::path::PathBuf::from("/bin/sh"),
            vec!["-c".into(), "seq 1 200000".into()],
        );
        let (sender, receiver) = async_channel::unbounded();
        let pty = Pty::spawn(
            &spec.shell,
            spec.working_directory,
            ScreenSize::new(40, 10, 200),
            sender,
            0,
            id,
            &[("TERM".into(), "xterm-256color".into())],
        )
        .expect("测试环境必须能派生 PTY");
        ptys.push(pty);
        receivers.push(receiver);
    }
    let deadline = Instant::now() + Duration::from_secs(90);
    let mut exits = [false; 3];
    let mut rounds = 0u32;
    loop {
        rounds += 1;
        for (index, receiver) in receivers.iter().enumerate() {
            while let Ok((_, event)) = receiver.try_recv() {
                if matches!(event, VtEvent::ChildExit(_)) {
                    exits[index] = true;
                }
            }
        }
        if exits.iter().all(|exited| *exited) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "90 秒内三个会话未全部退出：{exits:?}"
        );
        std::thread::sleep(Duration::from_millis(16));
    }
    for (index, pty) in ptys.iter().enumerate() {
        assert!(
            frame_text(pty).contains("200000"),
            "会话 {index} 的尾部数字必须在帧里"
        );
    }
    eprintln!("[p2-measure] 3 sessions seq-200k rounds={rounds} exits={exits:?}");
}

#[test]
fn wakeup_flood_drains_without_growth_or_loss() {
    use crate::io::vt::VtListener;
    use alacritty_terminal::event::{Event, EventListener};
    let (sender, receiver) = async_channel::unbounded();
    let listener = VtListener::new(sender, 0);
    let count = 50_000u32;
    for _ in 0..count {
        listener.send_event(Event::Wakeup);
    }
    assert_eq!(
        receiver.len(),
        count as usize,
        "洪泛的唤醒事件必须全在队列里，不能静默丢"
    );
    let started = Instant::now();
    let mut drained = 0u32;
    while receiver.try_recv().is_ok() {
        drained += 1;
    }
    assert_eq!(drained, count);
    eprintln!(
        "[p2-measure] wakeup-flood {count} events drained in {:?}",
        started.elapsed()
    );
}
