// @Time    : 2026/11/5 18:20
// @Author  : fzf
// @FileName: events.rs
// @Software: Siskin

//! PTY 派生与 VT 事件消费。所有会话共用一条事件通道，事件里带会话 id，
//! 这样关闭会话时不需要逐个注销，也不会在 id 复用时串线。

use gpui::Context;

use crate::domain::SessionSpec;
use crate::io::vt::VtEvent;

use super::SiskinApp;

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
                }
                continue;
            }
            let id = self.sessions.next_id();
            self.sessions.add(super::Session::starting(
                id,
                snapshot.title.clone(),
                self.settings.shell,
            ));
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
    pub(crate) fn drain_events(&mut self) -> bool {
        let mut changed = false;
        let mut exited: Vec<(u64, Option<i32>)> = Vec::new();
        let Some(receiver) = self.event_receiver.clone() else {
            return false;
        };
        while let Ok((id, event)) = receiver.try_recv() {
            changed = true;
            match event {
                VtEvent::Title(title) => self.apply_title(id, &title),
                VtEvent::ResetTitle => {
                    if let Some(index) = self.sessions.position(id)
                        && let Some(session) = self.sessions.get_mut(index)
                    {
                        session.window_title = None;
                    }
                }
                VtEvent::ClipboardStore(text) => self.clipboard_queue.push(text),
                VtEvent::ChildExit(code) => exited.push((id, code)),
                VtEvent::Exit => exited.push((id, None)),
                // 程序要求终端回话：直接写回同一个 PTY。
                VtEvent::PtyWrite(text) => {
                    if let Some(index) = self.sessions.position(id)
                        && let Some(session) = self.sessions.get_mut(index)
                    {
                        session.write(text.as_bytes());
                    }
                }
                VtEvent::Wakeup | VtEvent::Bell => {}
            }
        }
        let colors = self.ansi();
        for (id, code) in exited {
            let Some(index) = self.sessions.position(id) else {
                continue;
            };
            if let Some(session) = self.sessions.get_mut(index) {
                session.mark_exited(code);
            }
            changed = true;
            if self.exit_on_child_exit && index == self.active_index() {
                self.request_quit = true;
            }
        }
        for index in 0..self.sessions.len() {
            if let Some(session) = self.sessions.get_mut(index) {
                session.refresh(&colors);
            }
        }
        changed
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
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(16))
                    .await;
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
