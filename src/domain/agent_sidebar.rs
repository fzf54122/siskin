// @Time    : 2026/11/11
// @Author  : fzf
// @FileName: agent_sidebar.rs
// @Software: Siskin

//! Agent 侧栏的状态推导：只读会话状态与 OSC 133 命令记录，产出每条
//! 侧栏条目。不碰 PTY、GPUI 与文件；状态机与耗时格式都在这，单测覆盖。
//!
//! 只呈现已有会话的状态，不做 agent 运行时、不做并行监控后端。
//! 无标记的 shell（没装集成脚本）是合法态：没有待定命令、没有已完成
//! 命令时回退到「空闲」，不猜它在跑什么。

use std::time::{Duration, Instant};

use super::session::SessionState;
use super::shell_marks::CommandLog;

/// 侧栏刷新节流：render 每帧都到，但条目最多 250ms 重算一次，
/// 避免状态变化（每轮 PTY 输出）把帧率拖垮；开合与聚焦强制刷新。
pub(crate) const REFRESH_INTERVAL: Duration = Duration::from_millis(250);

/// 侧栏展开宽度（像素）。收起时不占宽，终端拿回全部宽度。
pub(crate) const AGENT_SIDEBAR_WIDTH: f32 = 260.;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EntryStatus {
    /// PTY 已请求派生，还没收到第一份输出。
    Starting,
    /// 有 `C` 没 `D`：命令正在跑。
    Running,
    /// 最近一条命令退出码为 0（缺码也算完成，只是不清楚码）。
    Succeeded,
    /// 最近一条命令退出码非 0。
    Failed,
    /// 运行中，但没有待定命令、也没有已完成命令：无标记 shell 的回退态。
    Idle,
    /// 子进程已退出。
    Exited,
    /// 会话派生失败。
    SpawnFailed,
}

/// 侧栏条目缓存：250ms 节流重算的状态。`Default` 是脏的，
/// 首帧强制算一次，否则收起时的失败红点永远是旧缓存。
#[derive(Debug)]
pub(crate) struct SidebarCache {
    pub entries: Vec<AgentEntry>,
    pub refreshed: Instant,
    pub dirty: bool,
}

impl Default for SidebarCache {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            refreshed: Instant::now(),
            dirty: true,
        }
    }
}

/// 一条侧栏条目：一个会话 / pane。`failed` 是收起时 tab 栏红点的依据。
#[derive(Clone, Debug)]
pub(crate) struct AgentEntry {
    pub session_id: u64,
    pub title: String,
    pub status: EntryStatus,
    /// 当前（运行中）或最近一条（已完成）命令；无标记时 `None`。
    pub command: Option<String>,
    /// 状态行文案：状态 + 耗时 / 退出码，如「运行中 · 1.2s」。
    pub detail: String,
    pub failed: bool,
}

fn entry(
    session_id: u64,
    title: String,
    status: EntryStatus,
    command: Option<String>,
    detail: String,
    failed: bool,
) -> AgentEntry {
    AgentEntry {
        session_id,
        title,
        status,
        command,
        detail,
        failed,
    }
}

/// 耗时后缀：有值时是「 · 1.2s」，没值时是空串（起止时间未知）。
fn duration_suffix(duration: Option<Duration>) -> String {
    match duration {
        Some(duration) => format!(" · {}", format_duration(duration)),
        None => String::new(),
    }
}

/// 从会话状态与命令记录推导一条侧栏条目。`now` 由调用方传入，
/// 这样单测可以固定时间断言已跑时长。
pub(crate) fn derive_entry(
    session_id: u64,
    title: String,
    state: &SessionState,
    log: &CommandLog,
    now: Instant,
) -> AgentEntry {
    match state {
        SessionState::Starting => entry(
            session_id,
            title,
            EntryStatus::Starting,
            None,
            "启动中".to_string(),
            false,
        ),
        SessionState::Failed(_) => entry(
            session_id,
            title,
            EntryStatus::SpawnFailed,
            None,
            "启动失败".to_string(),
            true,
        ),
        SessionState::Exited(info) => entry(
            session_id,
            title,
            EntryStatus::Exited,
            None,
            info.label(),
            info.code.is_some_and(|code| code != 0),
        ),
        SessionState::Running => derive_running(session_id, title, log, now),
    }
}

fn derive_running(session_id: u64, title: String, log: &CommandLog, now: Instant) -> AgentEntry {
    if let Some(pending) = log.pending_command() {
        let elapsed = now.saturating_duration_since(pending.started_at);
        return entry(
            session_id,
            title,
            EntryStatus::Running,
            pending.text,
            format!("运行中 · {}", format_duration(elapsed)),
            false,
        );
    }
    let Some(span) = log.last_span() else {
        return entry(
            session_id,
            title,
            EntryStatus::Idle,
            None,
            "空闲".into(),
            false,
        );
    };
    match span.exit_code {
        Some(0) => entry(
            session_id,
            title,
            EntryStatus::Succeeded,
            span.text.clone(),
            format!("完成{}", duration_suffix(span.duration)),
            false,
        ),
        Some(code) => entry(
            session_id,
            title,
            EntryStatus::Failed,
            span.text.clone(),
            format!("失败 · 退出 {code}{}", duration_suffix(span.duration)),
            true,
        ),
        // 缺码不是失败，只是不清楚：算完成，文案写明码未知。
        None => entry(
            session_id,
            title,
            EntryStatus::Succeeded,
            span.text.clone(),
            format!("完成 · 退出码未知{}", duration_suffix(span.duration)),
            false,
        ),
    }
}

/// 耗时格式：1 秒内按毫秒，1 分钟内一位小数秒，1 小时内分秒，之外时分。
pub(crate) fn format_duration(duration: Duration) -> String {
    let millis = duration.as_millis();
    if millis < 1_000 {
        format!("{millis}ms")
    } else if millis < 60_000 {
        format!("{:.1}秒", millis as f64 / 1_000.)
    } else if millis < 3_600_000 {
        let secs = millis / 1_000;
        format!("{}分{:02}秒", secs / 60, secs % 60)
    } else {
        let minutes = millis / 60_000;
        format!("{}小时{:02}分", minutes / 60, minutes % 60)
    }
}

/// 任一条目失败（命令失败、派生失败、非 0 退出）时侧栏收起也要给红点。
pub(crate) fn has_failure(entries: &[AgentEntry]) -> bool {
    entries.iter().any(|entry| entry.failed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::shell_marks::{Osc133Mark, ShellMark};

    fn mark(line: i32, kind: Osc133Mark) -> ShellMark {
        ShellMark::new(line, 0, kind)
    }

    fn run_cycle(log: &mut CommandLog, exit_code: Option<i32>, text: &str) {
        log.apply(mark(0, Osc133Mark::PromptStart));
        log.apply(mark(1, Osc133Mark::PreExec));
        log.note_pending_text(text);
        log.apply(mark(2, Osc133Mark::CommandEnd { exit_code }));
    }

    #[test]
    fn running_entry_shows_the_pending_command_and_elapsed_time() {
        let mut log = CommandLog::new();
        log.apply(mark(0, Osc133Mark::PromptStart));
        log.apply(mark(1, Osc133Mark::PreExec));
        log.note_pending_text("cargo test");
        let started = log.pending_command().unwrap().started_at;
        let entry = derive_entry(
            7,
            "终端".into(),
            &SessionState::Running,
            &log,
            started + Duration::from_millis(1_500),
        );
        assert_eq!(entry.status, EntryStatus::Running);
        assert_eq!(entry.command.as_deref(), Some("cargo test"));
        assert_eq!(entry.detail, "运行中 · 1.5秒");
        assert!(!entry.failed);
    }

    #[test]
    fn failed_entry_carries_the_exit_code_and_a_badge_flag() {
        let mut log = CommandLog::new();
        run_cycle(&mut log, Some(3), "false");
        let entry = derive_entry(
            7,
            "终端".into(),
            &SessionState::Running,
            &log,
            Instant::now(),
        );
        assert_eq!(entry.status, EntryStatus::Failed);
        assert_eq!(entry.command.as_deref(), Some("false"));
        assert!(
            entry.detail.starts_with("失败 · 退出 3"),
            "{}",
            entry.detail
        );
        assert!(entry.failed);
    }

    #[test]
    fn succeeded_entry_reports_duration_and_no_failure() {
        let mut log = CommandLog::new();
        run_cycle(&mut log, Some(0), "ls");
        let entry = derive_entry(
            7,
            "终端".into(),
            &SessionState::Running,
            &log,
            Instant::now(),
        );
        assert_eq!(entry.status, EntryStatus::Succeeded);
        assert!(entry.detail.starts_with("完成"), "{}", entry.detail);
        assert!(!entry.failed);
    }

    #[test]
    fn missing_exit_code_is_completed_not_failed() {
        let mut log = CommandLog::new();
        run_cycle(&mut log, None, "ls");
        let entry = derive_entry(
            7,
            "终端".into(),
            &SessionState::Running,
            &log,
            Instant::now(),
        );
        assert_eq!(entry.status, EntryStatus::Succeeded);
        assert!(entry.detail.contains("退出码未知"), "{}", entry.detail);
        assert!(!entry.failed);
    }

    #[test]
    fn a_shell_without_marks_falls_back_to_idle() {
        let log = CommandLog::new();
        let entry = derive_entry(
            7,
            "终端".into(),
            &SessionState::Running,
            &log,
            Instant::now(),
        );
        assert_eq!(entry.status, EntryStatus::Idle);
        assert_eq!(entry.command, None);
        assert_eq!(entry.detail, "空闲");
        assert!(!entry.failed);
    }

    #[test]
    fn session_level_states_map_to_their_own_entries() {
        let log = CommandLog::new();
        let now = Instant::now();
        let starting = derive_entry(1, "a".into(), &SessionState::Starting, &log, now);
        assert_eq!(starting.status, EntryStatus::Starting);
        assert!(!starting.failed);
        let failed = derive_entry(
            2,
            "b".into(),
            &SessionState::Failed("boom".into()),
            &log,
            now,
        );
        assert_eq!(failed.status, EntryStatus::SpawnFailed);
        assert_eq!(failed.detail, "启动失败");
        assert!(failed.failed);
        let exited_ok = derive_entry(
            3,
            "c".into(),
            &SessionState::Exited(crate::domain::ExitInfo::from_status(Some(0))),
            &log,
            now,
        );
        assert_eq!(exited_ok.status, EntryStatus::Exited);
        assert!(!exited_ok.failed);
        let exited_bad = derive_entry(
            4,
            "d".into(),
            &SessionState::Exited(crate::domain::ExitInfo::from_status(Some(1))),
            &log,
            now,
        );
        assert!(exited_bad.failed, "非 0 退出也要给红点");
    }

    #[test]
    fn durations_format_across_scales() {
        assert_eq!(format_duration(Duration::from_millis(0)), "0ms");
        assert_eq!(format_duration(Duration::from_millis(345)), "345ms");
        assert_eq!(format_duration(Duration::from_millis(1_250)), "1.2秒");
        assert_eq!(format_duration(Duration::from_millis(65_000)), "1分05秒");
        assert_eq!(
            format_duration(Duration::from_millis(3_661_000)),
            "1小时01分"
        );
    }

    #[test]
    fn has_failure_only_triggers_on_failed_entries() {
        let log = CommandLog::new();
        let now = Instant::now();
        let idle = derive_entry(1, "a".into(), &SessionState::Running, &log, now);
        assert!(!has_failure(&[idle]));
        let mut bad = CommandLog::new();
        run_cycle(&mut bad, Some(1), "false");
        let failed = derive_entry(2, "b".into(), &SessionState::Running, &bad, now);
        assert!(has_failure(&[failed]));
    }
}
