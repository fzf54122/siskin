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
fn closing_the_last_session_asks_the_window_to_close_instead_of_respawning() {
    // 剩一个会话时仓储拒绝删除：关闭由应用层转成「请求退出」，
    // 不会偷偷再派生一个 shell 让窗口永远关不掉。
    let mut set = sessions(1);
    assert!(!set.close(0));
    assert_eq!(set.len(), 1);
}

#[test]
fn closing_one_of_two_sessions_keeps_the_remaining_one_active() {
    let mut set = sessions(2);
    set.select(1);
    assert!(set.close(1));
    assert_eq!(set.len(), 1);
    assert_eq!(set.active_index(), 0);
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

/// 平滑滚动的行为契约。这些是回归护栏：把 `scroll_pixels` 改回
/// 「直接取整成行」或「每次都调 PTY」都会在这里挂掉。
#[test]
fn wheel_deltas_smaller_than_a_row_do_not_move_the_grid() {
    let mut session = Session::starting(0, "滚动", ShellKind::Zsh);
    // 19px 的行高，只给 7px：不该滚，但亚行余量要记下来供渲染平移。
    let moved = session.scroll_pixels(7., 19.);
    assert!(moved, "有位移就该重绘，否则亚行平移不连续");
    assert_eq!(session.scroll_offset.lines, 0);
    assert_eq!(session.scroll_offset.pending, 7.);
    assert_eq!(session.sub_pixel_offset(19.), 7.);
}

#[test]
fn a_row_worth_of_pixels_reaches_the_terminal_once() {
    let mut session = Session::starting(0, "滚动", ShellKind::Zsh);
    // 7 + 7 + 7 = 21px，行高 19px：第三下凑满一行，只提交 1 行。
    session.scroll_pixels(7., 19.);
    session.scroll_pixels(7., 19.);
    session.scroll_pixels(7., 19.);
    assert_eq!(session.scroll_offset.lines, 1);
    assert_eq!(session.scroll_offset.pending, 2.);
}

#[test]
fn opposite_wheel_deltas_cancel_without_touching_the_terminal() {
    let mut session = Session::starting(0, "滚动", ShellKind::Zsh);
    session.scroll_pixels(-7., 19.);
    session.scroll_pixels(7., 19.);
    assert_eq!(session.scroll_offset.lines, 0);
    assert_eq!(session.scroll_offset.pending, 0.);
    assert_eq!(session.sub_pixel_offset(19.), 0.);
}
