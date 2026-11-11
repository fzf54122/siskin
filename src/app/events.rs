// @Time    : 2026/11/5 18:20
// @Author  : fzf
// @FileName: events.rs
// @Software: Siskin

//! PTY 派生与 VT 事件消费。所有会话共用一条事件通道，事件里带会话 id，
//! 这样关闭会话时不需要逐个注销，也不会在 id 复用时串线。
//!
//! 事件分两类，处理方式不同，顺序都按通道到达顺序：
//! - 可合并：`Wakeup` / `Bell` 只把会话标脏，一轮里多个唤醒只构一次帧。
//! - 不可丢、不可乱序：`Title` / `ResetTitle` / `ClipboardStore` /
//!   `PtyWrite` / `ChildExit` / `Exit` / `ShellMark`，收到就按序处理，退出本轮末尾统一应用。
//!
//! 通道保持无界（`async_channel::unbounded`），是评估后的决定，不是
//! 遗留：发送端用 `try_send`，有界通道满了会静默丢事件——丢 `Wakeup`
//! 会让脏标记永远不置位、帧停在旧输出；丢标题、剪贴板、回写、退出
//! 则直接丢用户数据。事件本身很小（`Wakeup` 是空变体），消费端每轮
//! （默认 16ms）把队列排空，队列深度由「一轮内的 PTY 读取次数」决定，
//! 不是由总输出量决定；`seq 1 1000000` 的洪泛测试见 `tests/pty.rs`。

use std::time::Duration;

use gpui::Context;

use crate::domain::SessionSpec;
use crate::io::vt::VtEvent;

use super::SiskinApp;

/// 事件轮询间隔默认值（毫秒）。`SISKIN_POLL_MS` 可覆盖，用于
/// P2 的窗口实验；钳制在 2..=100，避免 0 忙轮询或过长拖键入延迟。
pub(crate) const DEFAULT_POLL_MS: u64 = 16;

/// 剪贴板写队列上限。同步时按顺序逐条写进系统剪贴板，最终生效的
/// 只有最后一条；恶意程序用 OSC 52 洪泛时丢最老的、只留最新这些，
/// 队列不会随输出无限增长。
pub(crate) const MAX_CLIPBOARD_WRITES: usize = 64;

/// 从环境解析轮询间隔：非法值、0、超出范围都回落到默认值或边界。
pub(crate) fn poll_interval() -> Duration {
    let millis = std::env::var("SISKIN_POLL_MS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(DEFAULT_POLL_MS)
        .clamp(2, 100);
    Duration::from_millis(millis)
}

impl SiskinApp {
    /// 应用启动后派生所有会话，并开始消费事件。
    ///
    /// 有上次会话快照时按快照恢复 cwd 与标题；`-e` 命令优先于快照。
    pub(crate) fn start_sessions(&mut self, cx: &mut Context<Self>) {
        let spec = self.pending_spec.take();
        if let Some(spec) = spec {
            let _ = self.spawn_spec(0, &spec);
        } else {
            self.restore_sessions();
            self.restore_pane_focus();
            self.session_scroll.scroll_to_item(self.active_index());
            for index in 0..self.sessions.len() {
                let working_directory = self.restored.get(index).and_then(|s| s.directory());
                let spec = SessionSpec::new(
                    self.sessions
                        .get(index)
                        .map(|session| session.title.to_string())
                        .unwrap_or_else(|| "终端".to_string()),
                    self.settings.shell,
                )
                .with_directory(working_directory);
                let _ = self.spawn_spec_from(index, &spec, true);
            }
        }
        self.listen_pty_events(cx);
    }

    /// 用快照重建会话列表。没有快照时保留启动时那一个空会话。
    fn restore_sessions(&mut self) {
        let snapshots = std::mem::take(&mut self.restored);
        if snapshots.is_empty() {
            return;
        }
        for (index, snapshot) in snapshots.iter().enumerate() {
            if index == 0 {
                if let Some(session) = self.sessions.get_mut(0) {
                    session.title = snapshot.title.clone().into();
                    session.custom_title = snapshot.custom_title.clone();
                }
                continue;
            }
            let id = self.sessions.next_id();
            let mut created =
                super::Session::starting(id, snapshot.title.clone(), self.settings.shell);
            created.custom_title = snapshot.custom_title.clone();
            self.sessions.add(created);
        }
        self.restored = snapshots;
    }

    fn new_event_channel(&mut self) -> async_channel::Sender<(u64, VtEvent)> {
        if let Some(sender) = self.event_sender.as_ref() {
            return sender.clone();
        }
        let (sender, receiver) = async_channel::unbounded();
        self.event_receiver = Some(receiver);
        self.event_sender = Some(sender.clone());
        sender
    }

    pub(crate) fn spawn_session(&mut self, index: usize) -> Result<(), String> {
        let Some(title) = self.sessions.get(index).map(|s| s.title.to_string()) else {
            return Err("会话不存在".to_string());
        };
        let directory = self
            .sessions
            .get(index)
            .and_then(|session| session.directory.clone());
        let spec = SessionSpec::new(title, self.settings.shell).with_directory(directory);
        self.spawn_spec(index, &spec)
    }

    fn spawn_spec(&mut self, index: usize, spec: &SessionSpec) -> Result<(), String> {
        self.spawn_spec_from(index, spec, false)
    }

    fn spawn_spec_from(
        &mut self,
        index: usize,
        spec: &SessionSpec,
        quiet: bool,
    ) -> Result<(), String> {
        let sender = self.new_event_channel();
        let window_id = self.window_id;
        let Some(session) = self.sessions.get_mut(index) else {
            return Err("会话不存在".to_string());
        };
        if let Some(directory) = spec.working_directory.clone() {
            session.directory = Some(directory);
        }
        match session.spawn_spec(spec, sender, window_id) {
            Ok(()) => {
                if !quiet {
                    self.status = None;
                }
                Ok(())
            }
            Err(error) => {
                if !quiet {
                    self.status = Some(error.clone());
                }
                // `-e` 启动失败不该让窗口退出，否则用户看不到原因。
                self.exit_on_child_exit = false;
                Err(error)
            }
        }
    }

    /// 消费一次事件。返回 `true` 表示界面需要重绘。
    /// P0 采样只在 `changed` 时记一条：空闲轮次不触发渲染，
    /// 记进来会把 FPS 虚报到 16ms 定时器的 62.5。
    pub(crate) fn drain_events(&mut self) -> bool {
        let started = std::time::Instant::now();
        let mut changed = false;
        let mut pty_events: u32 = 0;
        let mut exited: Vec<(u64, Option<i32>)> = Vec::new();
        let Some(receiver) = self.event_receiver.clone() else {
            return false;
        };
        while let Ok((id, event)) = receiver.try_recv() {
            changed = true;
            pty_events = pty_events.saturating_add(1);
            match event {
                VtEvent::Title(title) => self.apply_title(id, &title),
                VtEvent::ResetTitle => {
                    if let Some(index) = self.sessions.position(id)
                        && let Some(session) = self.sessions.get_mut(index)
                    {
                        session.window_title = None;
                    }
                }
                VtEvent::ClipboardStore(text) => self.push_clipboard_write(text),
                VtEvent::ChildExit(code) => exited.push((id, code)),
                VtEvent::Exit => exited.push((id, None)),
                // OSC 133 标记不可合并、不可乱序：按通道顺序进命令记录。
                // 同段输出的 Wakeup 随后就到，脏标记由它负责，这里不重复标。
                VtEvent::ShellMark(mark) => {
                    if let Some(index) = self.sessions.position(id)
                        && let Some(session) = self.sessions.get_mut(index)
                    {
                        session.apply_shell_mark(mark);
                    }
                }
                // 程序要求终端回话：直接写回同一个 PTY。
                VtEvent::PtyWrite(text) => {
                    if let Some(index) = self.sessions.position(id)
                        && let Some(session) = self.sessions.get_mut(index)
                    {
                        session.write(text.as_bytes());
                    }
                }
                // 唤醒意味着这个会话有新字节进了 VT 状态。构帧推迟到
                // 本轮回合末尾，一轮多个事件只构一次帧。
                VtEvent::Wakeup | VtEvent::Bell => self.mark_session_output(id),
            }
        }
        let colors = self.ansi();
        changed = changed || self.apply_exits(exited);
        let (sessions_refreshed, frame_build) = self.refresh_dirty_sessions(&colors);
        changed = changed || sessions_refreshed > 0;
        if sessions_refreshed > 0 {
            // 新 PTY 输出到达且搜索条打开时重算；帧没变时缓存守卫会
            // 直接复用，不会每轮都重扫历史。
            self.refresh_search();
        }
        if changed {
            self.record_perf(started, frame_build, pty_events, sessions_refreshed);
        }
        changed
    }

    /// 剪贴板写请求进队，超上限时丢最老的一条。同步时按序写进系统
    /// 剪贴板，只有最后一条真正生效，丢旧的不影响最终内容。
    fn push_clipboard_write(&mut self, text: String) {
        if self.clipboard_queue.len() >= MAX_CLIPBOARD_WRITES {
            self.clipboard_queue.remove(0);
        }
        self.clipboard_queue.push(text);
    }

    /// 只刷新这一轮真有输出的会话。空闲会话的 Frame 是准确的，
    /// 重新构一次纯属白锁 PTY、白拷贝一帧网格。
    fn refresh_dirty_sessions(
        &mut self,
        colors: &crate::io::ansi::AnsiColors,
    ) -> (u32, std::time::Duration) {
        let mut refreshed: u32 = 0;
        let mut build = std::time::Duration::ZERO;
        for index in 0..self.sessions.len() {
            if let Some(session) = self.sessions.get_mut(index)
                && session.frame_dirty()
            {
                build += session.refresh(colors);
                refreshed = refreshed.saturating_add(1);
            }
        }
        (refreshed, build)
    }

    fn apply_exits(&mut self, exited: Vec<(u64, Option<i32>)>) -> bool {
        let mut changed = false;
        for (id, code) in exited {
            let Some(index) = self.sessions.position(id) else {
                continue;
            };
            if let Some(session) = self.sessions.get_mut(index) {
                session.mark_exited(code);
                session.mark_output();
            }
            changed = true;
            if self.exit_on_child_exit && index == self.active_index() {
                self.request_quit = true;
            }
        }
        changed
    }

    fn record_perf(
        &self,
        started: std::time::Instant,
        frame_build: std::time::Duration,
        pty_events: u32,
        sessions_refreshed: u32,
    ) {
        if !crate::domain::perf::is_enabled() {
            return;
        }
        // 缓存命中数由 prepaint 补进同一条样本：app 层不反向依赖 ui。
        crate::domain::perf::record(crate::domain::perf::PerfSample {
            frame_time: started.elapsed(),
            shape_time: std::time::Duration::ZERO,
            frame_build_time: frame_build,
            pty_events,
            sessions_refreshed,
            draw_tasks: 0,
            shape_hits: 0,
            shape_misses: 0,
        });
    }

    /// 标记某个会话有新输出。会话已关闭时静默忽略：关闭与事件可能并发。
    fn mark_session_output(&mut self, id: u64) {
        if let Some(index) = self.sessions.position(id)
            && let Some(session) = self.sessions.get_mut(index)
        {
            session.mark_output();
        }
    }

    /// OSC 0/2 标题。只影响窗口标题与标题栏副标题，不改会话名：
    /// 否则 `vim` 打开文件、`ssh` 登录都会把标签改名，用户找不到会话。
    fn apply_title(&mut self, id: u64, title: &str) {
        if self.sessions.position(id).is_none() {
            return;
        }
        if let Some(index) = self.sessions.position(id)
            && let Some(session) = self.sessions.get_mut(index)
        {
            session.window_title = Some(title.to_string()).filter(|title| !title.is_empty());
        }
    }

    /// 事件循环在应用层的主任务里跑，而不是每帧轮询。
    pub(crate) fn listen_pty_events(&mut self, cx: &mut Context<Self>) {
        if self.event_listening {
            return;
        }
        self.event_listening = true;
        let interval = poll_interval();
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(interval).await;
                let alive = this
                    .update(cx, |app, cx| {
                        let changed = app.drain_events();
                        let saved = app.flush_preferences_if_due();
                        if app.request_quit {
                            cx.quit();
                            return false;
                        }
                        if changed || saved {
                            cx.notify();
                        }
                        true
                    })
                    .unwrap_or(false);
                if !alive {
                    break;
                }
            }
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_POLL_MS, poll_interval};
    use std::time::Duration;

    /// 环境变量是进程全局的，多个用例并行读写会互相干扰，所以
    /// 全部解析分支放在一个测试里顺序跑。
    #[test]
    fn poll_interval_parses_and_clamps_the_env_override() {
        // SAFETY: 本测试文件里只有这一个测试碰 SISKIN_POLL_MS。
        unsafe { std::env::remove_var("SISKIN_POLL_MS") };
        assert_eq!(poll_interval(), Duration::from_millis(DEFAULT_POLL_MS));
        unsafe { std::env::set_var("SISKIN_POLL_MS", "4") };
        assert_eq!(poll_interval(), Duration::from_millis(4));
        unsafe { std::env::set_var("SISKIN_POLL_MS", " 2 ") };
        assert_eq!(poll_interval(), Duration::from_millis(2));
        unsafe { std::env::set_var("SISKIN_POLL_MS", "0") };
        assert_eq!(poll_interval(), Duration::from_millis(2), "0 钳到下限");
        unsafe { std::env::set_var("SISKIN_POLL_MS", "5000") };
        assert_eq!(poll_interval(), Duration::from_millis(100), "超大钳到上限");
        unsafe { std::env::set_var("SISKIN_POLL_MS", "abc") };
        assert_eq!(
            poll_interval(),
            Duration::from_millis(DEFAULT_POLL_MS),
            "非法值回落默认"
        );
        unsafe { std::env::remove_var("SISKIN_POLL_MS") };
    }
}
