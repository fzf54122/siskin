// @Time    : 2026/11/11
// @Author  : fzf
// @FileName: agent_sidebar.rs
// @Software: Siskin

//! Agent 侧栏：开合、状态推导、点击聚焦、失败徽章。
//!
//! 不走真实 PTY：会话用 `Session::starting` 直建，OSC 133 标记直接喂
//! `apply_shell_mark`；真窗口交互（开合、点击条目、红点）走
//! `VisualTestContext`，断言 debug selector 出现与焦点变化。

use gpui::{AppContext as _, Entity, Modifiers, TestAppContext, VisualTestContext};

use super::views::{click, read, redraw, window};
use crate::app::{Handled, Session, SiskinApp};
use crate::domain::SessionState;
use crate::domain::ShellKind;
use crate::domain::agent_sidebar::EntryStatus;
use crate::domain::shell_marks::{Osc133Mark, ShellMark};

fn app(cx: &mut TestAppContext) -> Entity<SiskinApp> {
    cx.new(SiskinApp::new)
}

fn press(text: &str) -> gpui::Keystroke {
    gpui::Keystroke::parse(text).expect("测试里的按键写法必须合法")
}

/// 动态 debug selector（`agent-entry-{id}`）要 `&'static str`：
/// 测完即弃，leak 一小段字符串换一次断言。
fn leak_selector(selector: &str) -> &'static str {
    Box::leak(selector.to_string().into_boxed_str())
}

fn click_selector(cx: &mut VisualTestContext, selector: &str) {
    redraw(cx);
    let selector = leak_selector(selector);
    let bounds = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("missing element {selector}"));
    cx.simulate_click(bounds.center(), Modifiers::default());
    redraw(cx);
}

fn mark(session: &mut Session, line: i32, kind: Osc133Mark) {
    session.apply_shell_mark(ShellMark::new(line, 0, kind));
}

/// 喂一个失败命令（退出 3）加一个运行中命令，返回两个会话的 id。
/// 第一个会话本来就在仓储里，第二个直建，不派生 PTY。
/// 直建会话先把状态置为 `Running`：生产中标记只在派生成功后到达，
/// 收到标记的会话不可能是 `Starting`。
fn two_sessions(app: &mut SiskinApp) -> (u64, u64) {
    let first = app.sessions.items[0].id;
    app.sessions.items[0].state = SessionState::Running;
    mark(&mut app.sessions.items[0], 0, Osc133Mark::PromptStart);
    mark(&mut app.sessions.items[0], 1, Osc133Mark::PreExec);
    mark(
        &mut app.sessions.items[0],
        2,
        Osc133Mark::CommandEnd { exit_code: Some(3) },
    );
    let second = app.sessions.next_id();
    let mut created = Session::starting(second, "第二个", ShellKind::Zsh);
    created.state = SessionState::Running;
    app.sessions.add(created);
    let index = app.sessions.position(second).unwrap();
    mark(&mut app.sessions.items[index], 0, Osc133Mark::PromptStart);
    mark(&mut app.sessions.items[index], 1, Osc133Mark::PreExec);
    (first, second)
}

#[gpui::test]
fn toggle_sidebar_via_shortcut(cx: &mut TestAppContext) {
    let app = app(cx);
    app.update(cx, |app, _| {
        assert!(!app.agent_sidebar_open());
        assert!(matches!(
            app.handle_key(&press("ctrl-shift-a")),
            Some(Handled::Redraw)
        ));
        assert!(app.agent_sidebar_open());
        assert!(matches!(
            app.handle_key(&press("ctrl-shift-a")),
            Some(Handled::Redraw)
        ));
        assert!(!app.agent_sidebar_open());
    });
}

#[gpui::test]
fn entries_follow_marks_and_sessions_stay_in_sync(cx: &mut TestAppContext) {
    let app = app(cx);
    app.update(cx, |app, _| {
        let (first, second) = two_sessions(app);
        app.refresh_agent_sidebar();
        assert_eq!(app.agent_sidebar_cache.entries.len(), 2);
        let failed = app
            .agent_sidebar_cache
            .entries
            .iter()
            .find(|entry| entry.session_id == first)
            .unwrap();
        assert_eq!(failed.status, EntryStatus::Failed);
        assert!(failed.failed);
        let running = app
            .agent_sidebar_cache
            .entries
            .iter()
            .find(|entry| entry.session_id == second)
            .unwrap();
        assert_eq!(running.status, EntryStatus::Running);
        assert!(app.agent_sidebar_has_failure());
        // 同一会话再跑一条成功命令：失败红点消失。
        let index = app.sessions.position(first).unwrap();
        mark(&mut app.sessions.items[index], 3, Osc133Mark::PromptStart);
        mark(&mut app.sessions.items[index], 4, Osc133Mark::PreExec);
        mark(
            &mut app.sessions.items[index],
            5,
            Osc133Mark::CommandEnd { exit_code: Some(0) },
        );
        app.agent_sidebar_cache.dirty = true;
        app.refresh_agent_sidebar();
        assert!(!app.agent_sidebar_has_failure());
        // 关掉第二个会话：条目同步减少，不留幽灵条目。
        let index = app.sessions.position(second).unwrap();
        assert!(app.sessions.close(index));
        app.agent_sidebar_cache.dirty = true;
        app.refresh_agent_sidebar();
        assert_eq!(app.agent_sidebar_cache.entries.len(), 1);
        assert_eq!(app.agent_sidebar_cache.entries[0].session_id, first);
    });
}

#[gpui::test]
fn focusing_an_unknown_session_is_a_noop(cx: &mut TestAppContext) {
    let app = app(cx);
    app.update(cx, |app, _| {
        app.focus_agent_session(999);
        assert_eq!(app.active_index(), 0);
        assert_eq!(app.active_pane, 0);
    });
}

#[gpui::test]
fn sidebar_opens_closes_and_click_focuses_entries(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    redraw(cx);
    assert!(!read(&view, cx, |app| app.agent_sidebar_open()));
    assert!(cx.debug_bounds("agent-sidebar").is_none());
    cx.simulate_keystrokes("ctrl-shift-a");
    assert!(read(&view, cx, |app| app.agent_sidebar_open()));
    redraw(cx);
    assert!(cx.debug_bounds("agent-sidebar").is_some());
    // 第二个会话直接建（不派生 PTY），失败徽章与点击链路照常用。
    let second = view.update(cx, |app, _| {
        let id = app.sessions.next_id();
        let mut created = Session::starting(id, "第二个", ShellKind::Zsh);
        created.state = SessionState::Running;
        app.sessions.add(created);
        let index = app.sessions.position(id).unwrap();
        mark(&mut app.sessions.items[index], 0, Osc133Mark::PromptStart);
        mark(&mut app.sessions.items[index], 1, Osc133Mark::PreExec);
        mark(
            &mut app.sessions.items[index],
            2,
            Osc133Mark::CommandEnd { exit_code: Some(1) },
        );
        app.select_session(0);
        // 仓储变了但节流窗口没到：强制下一次重算，否则条目还是旧的。
        app.agent_sidebar_cache.dirty = true;
        id
    });
    redraw(cx);
    assert!(
        cx.debug_bounds(leak_selector(&format!("agent-entry-{second}")))
            .is_some(),
        "第二个会话的条目应当出现"
    );
    assert!(
        cx.debug_bounds(leak_selector(&format!("agent-failed-badge-{second}")))
            .is_some(),
        "失败命令的条目应当有失败徽章"
    );
    click_selector(cx, &format!("agent-entry-{second}"));
    assert_eq!(read(&view, cx, |app| app.active_index()), 1);
    assert_eq!(read(&view, cx, |app| app.active_pane), second);
    // 头部关闭按钮与 tab 栏开关都走同一开关。
    click(cx, "close-agent-sidebar");
    assert!(!read(&view, cx, |app| app.agent_sidebar_open()));
    assert!(cx.debug_bounds("agent-sidebar").is_none());
    click(cx, "toggle-agent-sidebar");
    assert!(read(&view, cx, |app| app.agent_sidebar_open()));
    assert!(cx.debug_bounds("agent-sidebar").is_some());
}
