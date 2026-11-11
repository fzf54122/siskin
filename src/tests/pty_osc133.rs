// @Time    : 2026/11/11
// @Author  : fzf
// @FileName: pty_osc133.rs
// @Software: Siskin

//! OSC 133 集成测试：真实 PTY 里的标记、行号锚定、命令跳转与 Bash/Zsh
//! 集成脚本验证。`spawn` / `wait_for_output` 复用 `tests/pty.rs`。

use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::domain::session::SessionSpec;
use crate::domain::shell_marks::{Osc133Mark, ShellMark};
use crate::domain::shell_spec::ShellKind;
use crate::io::ansi::AnsiColors;
use crate::io::vt::VtEvent;
use crate::theme::PAPER;

use super::pty::{session_text, spawn, wait_for_output};
/// 从事件通道收 ShellMark，直到 `want` 成立或超时。返回收到的全部标记。
/// OSC 标记与普通输出走同一条通道，不 poll 事件就永远看不到它们。
fn collect_marks(
    events: &async_channel::Receiver<(u64, VtEvent)>,
    seconds: u64,
    want: impl Fn(&[ShellMark]) -> bool,
) -> Vec<ShellMark> {
    let deadline = Instant::now() + Duration::from_secs(seconds);
    let mut marks = Vec::new();
    while Instant::now() < deadline {
        while let Ok((_, event)) = events.try_recv() {
            if let VtEvent::ShellMark(mark) = event {
                marks.push(mark);
            }
        }
        if want(&marks) {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    marks
}

/// OSC 133 全流程：真实 PTY 里 `sh` 发 A/B/C/D，命令记录里有一条
/// exit_code=Some(3) 的命令，起止行锚定正确，普通输出仍进网格。
#[test]
fn osc133_marks_land_in_the_session_command_log_with_exit_code() {
    let spec = SessionSpec::command(
        "test",
        std::path::PathBuf::from("/bin/sh"),
        vec![
            "-c".into(),
            "printf '\\033]133;A\\033\\\\P\\033]133;B\\033\\\\c\\033]133;C\\033\\\\out\\n\\033]133;D;3\\033\\\\'; sleep 2".into(),
        ],
    );
    let palette = AnsiColors::from_palette(&PAPER);
    let mut session = crate::app::Session::starting(30, "OSC 会话", ShellKind::Sh);
    let (sender, receiver) = async_channel::unbounded();
    session
        .spawn_spec(&spec, sender, 0)
        .expect("测试环境必须能派生 PTY");
    let marks = collect_marks(&receiver, 15, |marks| {
        marks
            .iter()
            .any(|mark| matches!(mark.mark, Osc133Mark::CommandEnd { .. }))
    });
    for mark in marks {
        session.apply_shell_mark(mark);
    }
    assert_eq!(session.shell_marks.len(), 1, "A/B/C/D 应合成一条命令");
    let span = &session.shell_marks.spans()[0];
    assert_eq!(span.exit_code, Some(3), "退出码只认 D 标记的参数");
    assert!(span.failed(), "exit 3 是失败命令");
    assert_eq!(span.start_line, 0, "A 在首行发出");
    assert_eq!(span.end_line, 1, "D 在换行后那一行发出");
    assert!(span.duration.is_some());
    // 标记字节照常进了 VT 解析器，普通输出不能丢。
    session.mark_output();
    session.refresh(&palette);
    assert!(
        session_text(&session).contains("out"),
        "OSC 标记不能剥离普通输出"
    );
}

/// 锚定行是稳定行号：滚进历史 5 行后再到的 D 标记，
/// 行号 = 当前 history_size +（光标视口行 - 5）。
#[test]
fn osc133_mark_line_is_absolute_when_scrolled_into_history() {
    let spec = SessionSpec::command(
        "test",
        std::path::PathBuf::from("/bin/sh"),
        vec![
            "-c".into(),
            "printf 'ready\\n'; read go; seq 1 15; sleep 0.5; printf '\\033]133;D;3\\033\\\\'; sleep 2".into(),
        ],
    );
    let (mut pty, events) = spawn(spec);
    assert!(wait_for_output(&pty, "ready"), "子进程未进入就绪状态");
    pty.write(b"\n");
    assert!(wait_for_output(&pty, "15"), "未读到 seq 输出");
    pty.scroll(5);
    assert_eq!(pty.display_offset(), 5, "应已滚进历史 5 行");
    let marks = collect_marks(&events, 15, |marks| {
        marks
            .iter()
            .any(|mark| matches!(mark.mark, Osc133Mark::CommandEnd { .. }))
    });
    assert_eq!(marks.len(), 1, "只发了一个 D 标记");
    let mark = marks[0];
    assert_eq!(mark.mark, Osc133Mark::CommandEnd { exit_code: Some(3) });
    // 光标在最后一行（视口行 9），滚了 5 行：绝对行 4，稳定行再加 history。
    assert_eq!(
        mark.line,
        pty.history_size() as i32 + 4,
        "锚定行必须减掉 display_offset"
    );
}

/// 没有标记的普通 shell：命令记录为空，终端正常工作。
#[test]
fn a_shell_without_marks_leaves_the_command_log_empty() {
    let spec = SessionSpec::command(
        "test",
        std::path::PathBuf::from("/bin/sh"),
        vec!["-c".into(), "printf 'plain'; sleep 1".into()],
    );
    let (pty, events) = spawn(spec);
    assert!(wait_for_output(&pty, "plain"), "未读到命令输出");
    let deadline = Instant::now() + Duration::from_millis(300);
    while Instant::now() < deadline {
        if let Ok((_, VtEvent::ShellMark(_))) = events.try_recv() {
            panic!("没有标记的 shell 不应产生 ShellMark");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// 命令间跳转：两条命令各隔 25 行，从底部上跳、下跳，目标命令滚到顶行。
#[test]
fn jumping_between_commands_scrolls_to_the_command_start() {
    let spec = SessionSpec::command(
        "test",
        std::path::PathBuf::from("/bin/sh"),
        vec![
            "-c".into(),
            "printf '\\033]133;A\\033\\\\\\033]133;B\\033\\\\\\033]133;C\\033\\\\'; seq 1 25; printf '\\033]133;D;0\\033\\\\'; printf '\\033]133;A\\033\\\\\\033]133;B\\033\\\\\\033]133;C\\033\\\\'; seq 26 50; printf '\\033]133;D;1\\033\\\\'; sleep 2".into(),
        ],
    );
    let mut session = crate::app::Session::starting(31, "跳转会话", ShellKind::Sh);
    let (sender, receiver) = async_channel::unbounded();
    session
        .spawn_spec(&spec, sender, 0)
        .expect("测试环境必须能派生 PTY");
    let marks = collect_marks(&receiver, 15, |marks| marks.len() >= 8);
    for mark in marks {
        session.apply_shell_mark(mark);
    }
    assert_eq!(session.shell_marks.len(), 2, "应有两条完整命令");
    assert_eq!(session.shell_marks.spans()[0].exit_code, Some(0));
    assert_eq!(session.shell_marks.spans()[1].exit_code, Some(1));
    assert!(session.shell_marks.spans()[1].failed());
    let history = session.history_size().expect("有 PTY") as i32;
    let first = session.shell_marks.spans()[0].start_line;
    let second = session.shell_marks.spans()[1].start_line;

    assert!(session.jump_command(false), "底部上跳应找到最新命令");
    assert_eq!(
        session.display_offset(),
        Some((history - second) as usize),
        "最新命令应滚到可见区顶部"
    );
    assert!(session.jump_command(false), "再上跳应找到第一条命令");
    assert_eq!(
        session.display_offset(),
        Some((history - first) as usize),
        "第一条命令应滚到可见区顶部"
    );
    assert!(session.jump_command(true), "下跳应回到第二条命令");
    assert_eq!(
        session.display_offset(),
        Some((history - second) as usize),
        "下跳应回到第二条命令的起始行"
    );
    let offset = session.display_offset();
    assert!(!session.jump_command(true), "已是最新命令时下跳应不动");
    assert_eq!(session.display_offset(), offset, "下跳失败时不改滚动位置");
}

/// 真实 Bash：按 `docs/shell-integration.md` 的片段配置交互式 bash，
/// 跑 `false`，应看到带退出码 1 的 D 标记与完整 A/B/C/D 子序列。
/// Bash 的 C 用 DEBUG trap 实现，PROMPT_COMMAND 内部命令会再触发一
/// 次 C，所以这里只断言子序列，不数 C 的个数。
#[test]
fn osc133_bash_integration_reports_the_command_exit_code() {
    let spec = SessionSpec::command(
        "test",
        std::path::PathBuf::from("/bin/bash"),
        vec!["--noprofile".into(), "--norc".into(), "-i".into()],
    );
    let (mut pty, events) = spawn(spec);
    let setup = [
        "__siskin_preexec() { printf '\\033]133;C\\033\\\\'; }\n",
        "trap '__siskin_preexec' DEBUG\n",
        "__siskin_prompt_command() { local code=$?; printf '\\033]133;D;%s\\033\\\\' \"$code\"; printf '\\033]133;A\\033\\\\'; }\n",
        "PROMPT_COMMAND=__siskin_prompt_command\n",
        "PS1='\\u@\\h:\\w\\$ \\[\\e]133;B\\e\\\\\\]'\n",
    ];
    for line in setup {
        pty.write(line.as_bytes());
        std::thread::sleep(Duration::from_millis(200));
    }
    std::thread::sleep(Duration::from_millis(500));
    while events.try_recv().is_ok() {}
    pty.write(b"false\n");
    let marks = collect_marks(&events, 15, |marks| {
        marks
            .iter()
            .any(|mark| mark.mark == Osc133Mark::CommandEnd { exit_code: Some(1) })
    });
    assert_subsequence(
        &marks,
        &[
            Osc133Mark::PreExec,
            Osc133Mark::CommandEnd { exit_code: Some(1) },
            Osc133Mark::PromptStart,
            Osc133Mark::CommandInput,
        ],
        "bash 跑 false 应有 C → D;1 → A → B",
    );
}

/// 真实 Zsh：按 `docs/shell-integration.md` 的片段配置交互式 zsh，
/// 跑 `false`，应看到带退出码 1 的 D 标记与干净的 A/B/C/D 子序列。
#[test]
fn osc133_zsh_integration_reports_the_command_exit_code() {
    let spec = SessionSpec::command(
        "test",
        std::path::PathBuf::from("/bin/zsh"),
        vec!["-i".into()],
    );
    let (mut pty, events) = spawn(spec);
    let setup = [
        "__siskin_precmd() { printf '\\033]133;D;%s\\033\\\\' \"$?\"; printf '\\033]133;A\\033\\\\'; }\n",
        "__siskin_preexec() { printf '\\033]133;C\\033\\\\'; }\n",
        "autoload -Uz add-zsh-hook\n",
        "add-zsh-hook precmd __siskin_precmd\n",
        "add-zsh-hook preexec __siskin_preexec\n",
        "PS1=$'%n@%m %~ %# \\x1b]133;B\\x1b\\\\'\n",
    ];
    for line in setup {
        pty.write(line.as_bytes());
        std::thread::sleep(Duration::from_millis(200));
    }
    std::thread::sleep(Duration::from_millis(500));
    while events.try_recv().is_ok() {}
    pty.write(b"false\n");
    let marks = collect_marks(&events, 15, |marks| {
        marks
            .iter()
            .any(|mark| mark.mark == Osc133Mark::CommandEnd { exit_code: Some(1) })
    });
    assert_subsequence(
        &marks,
        &[
            Osc133Mark::PreExec,
            Osc133Mark::CommandEnd { exit_code: Some(1) },
            Osc133Mark::PromptStart,
            Osc133Mark::CommandInput,
        ],
        "zsh 跑 false 应有 C → D;1 → A → B",
    );
}

/// 标记序列里按顺序出现 `wanted` 的每一项（中间可有其它标记）。
fn assert_subsequence(marks: &[ShellMark], wanted: &[Osc133Mark], message: &str) {
    let mut rest = marks.iter().map(|mark| mark.mark);
    for want in wanted {
        assert!(
            rest.any(|mark| mark == *want),
            "{message}，实际序列：{marks:?}"
        );
    }
}

/// `C` 到达时把刚提交的命令记进建议来源：字节顺序按真实 shell——
/// 提示符 `P` → `B` → 输入 → 回车换行 → `C` → 输出 → `D`。
/// 输入行在 `C` 到达时已经封行，输出落在下一行，捕获读到的就是完整命令。
#[test]
fn preexec_captures_the_submitted_command_for_suggestions() {
    let spec = SessionSpec::command(
        "test",
        PathBuf::from("/bin/sh"),
        vec![
            "-c".into(),
            "printf '\\033]133;A\\033\\\\P\\033]133;B\\033\\\\git status\\r\\n\\033]133;C\\033\\\\out\\n\\033]133;D;0\\033\\\\'; sleep 2".into(),
        ],
    );
    let palette = AnsiColors::from_palette(&PAPER);
    let mut session = crate::app::Session::starting(31, "Ghost 会话", ShellKind::Sh);
    let (sender, receiver) = async_channel::unbounded();
    session
        .spawn_spec(&spec, sender, 0)
        .expect("测试环境必须能派生 PTY");
    let marks = collect_marks(&receiver, 15, |marks| {
        marks
            .iter()
            .any(|mark| matches!(mark.mark, Osc133Mark::CommandEnd { .. }))
    });
    assert_eq!(marks.len(), 4, "应收到 A/B/C/D 各一个");
    let b = marks
        .iter()
        .find(|mark| matches!(mark.mark, Osc133Mark::CommandInput))
        .expect("应有 B 标记");
    assert_eq!(b.line, 0, "B 在提示符行");
    assert_eq!(b.column, 1, "B 到达时光标在提示符 P 之后");
    for mark in marks {
        session.apply_shell_mark(mark);
    }
    assert_eq!(session.shell_marks.len(), 1);
    assert_eq!(
        session.shell_marks.spans()[0].input_column,
        1,
        "B 的列应进命令记录"
    );
    assert_eq!(
        session.ghost.history.suggest("git "),
        Some("status".to_string()),
        "C 到达时应把输入行记进建议来源"
    );
    assert_eq!(
        session.ghost.history.suggest("git s"),
        Some("tatus".to_string())
    );
    // 标记字节照常进了 VT 解析器，普通输出不能丢。
    session.mark_output();
    session.refresh(&palette);
    assert!(
        session_text(&session).contains("out"),
        "OSC 标记不能剥离普通输出"
    );
}

/// 没有 `B` 就不知道提示符末尾：`C` 到了也不猜、不记。
#[test]
fn preexec_without_a_prompt_anchor_records_nothing() {
    let spec = SessionSpec::command(
        "test",
        PathBuf::from("/bin/sh"),
        vec![
            "-c".into(),
            "printf 'git status\\r\\n\\033]133;C\\033\\\\out\\n\\033]133;D;0\\033\\\\'; sleep 2"
                .into(),
        ],
    );
    let mut session = crate::app::Session::starting(32, "Ghost 会话", ShellKind::Sh);
    let (sender, receiver) = async_channel::unbounded();
    session
        .spawn_spec(&spec, sender, 0)
        .expect("测试环境必须能派生 PTY");
    let marks = collect_marks(&receiver, 15, |marks| {
        marks
            .iter()
            .any(|mark| matches!(mark.mark, Osc133Mark::CommandEnd { .. }))
    });
    for mark in marks {
        session.apply_shell_mark(mark);
    }
    assert_eq!(session.shell_marks.len(), 1, "没有 B 也能记一条命令");
    assert_eq!(
        session.ghost.history.suggest("git "),
        None,
        "不知道提示符末尾时不猜、不记"
    );
}
