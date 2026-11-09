// @Time    : 2026/11/5 18:45
// @Author  : fzf
// @FileName: session.rs
// @Software: Siskin

//! 会话仓储的边界行为。`SiskinApp` 只通过 `Sessions` 操作会话列表，
//! 所以这些规则在应用层之外单独验证更快也更稳。

use crate::app::{Session, Sessions};
use crate::domain::SessionState;
use crate::domain::shell_spec::ShellKind;

fn sessions(count: usize) -> Sessions {
    let mut set = Sessions::new(Session::starting(0, "第一个", ShellKind::Zsh));
    for index in 1..count {
        let id = set.next_id();
        set.add(Session::starting(
            id,
            format!("会话 {index}"),
            ShellKind::Zsh,
        ));
    }
    set
}

#[test]
fn adding_selects_the_new_session() {
    let mut set = sessions(1);
    let id = set.next_id();
    assert_eq!(set.add(Session::starting(id, "第二个", ShellKind::Bash)), 1);
    assert_eq!(set.len(), 2);
    assert_eq!(set.active_index(), 1);
    assert_eq!(set.active().kind, ShellKind::Bash);
}

#[test]
fn out_of_range_selection_and_close_are_ignored() {
    let mut set = sessions(2);
    let active = set.active_index();
    assert!(!set.select(5));
    assert_eq!(set.active_index(), active);
    assert!(!set.close(5));
    assert_eq!(set.len(), 2);
}

#[test]
fn closing_before_the_active_session_shifts_the_index() {
    let mut set = sessions(3);
    set.select(2);
    assert!(set.close(0));
    assert_eq!(set.len(), 2);
    assert_eq!(set.active_index(), 1);
    assert_eq!(
        set.get(1).map(|session| session.title.to_string()),
        Some("会话 2".into())
    );
}

#[test]
fn closing_the_active_tail_lands_on_the_previous_session() {
    let mut set = sessions(3);
    assert!(set.close(2));
    assert_eq!(set.len(), 2);
    assert_eq!(set.active_index(), 1);
}

#[test]
fn new_sessions_start_in_the_starting_state() {
    let set = sessions(1);
    assert_eq!(set.active().state, SessionState::Starting);
    assert!(!set.active().is_running());
}

#[test]
fn session_ids_survive_close_and_add() {
    let mut set = sessions(2);
    let id = set.next_id();
    set.add(Session::starting(id, "新会话", ShellKind::Bash));
    set.close(0);
    let ids: Vec<u64> = set.iter().map(|(_, session)| session.id).collect();
    assert_eq!(ids.len(), 2);
    assert_ne!(ids[0], ids[1]);
    assert!(set.position(ids[0]).is_some());
}
