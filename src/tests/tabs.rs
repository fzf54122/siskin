// @Time    : 2026/11/11
// @Author  : fzf
// @FileName: tabs.rs
// @Software: Siskin

//! 标签拖拽排序与重命名的集成测试：走真实渲染与鼠标/按键链路。
//! 拖拽是手动鼠标方案（`tab_ops.rs`），这里用 `simulate_mouse_*`
//! 按真实手势走一遍；分屏回归断言移动的是向量顺序、
//! 不是会话 id，`session()` 仍指向原焦点 pane。

use super::views::{click, read, redraw, wait_until, window};
use crate::domain::Action;
use gpui::{Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, TestAppContext};

fn double_click(cx: &mut gpui::VisualTestContext, at: gpui::Point<gpui::Pixels>) {
    // `ClickEvent::click_count` 取自 mouse-up 事件的 `click_count`，
    // 测试里直接派发带计数的原始事件，模拟真实双击。
    cx.simulate_event(MouseDownEvent {
        position: at,
        modifiers: Modifiers::default(),
        button: MouseButton::Left,
        click_count: 1,
        first_mouse: false,
    });
    cx.simulate_event(MouseUpEvent {
        position: at,
        modifiers: Modifiers::default(),
        button: MouseButton::Left,
        click_count: 1,
    });
    cx.simulate_event(MouseDownEvent {
        position: at,
        modifiers: Modifiers::default(),
        button: MouseButton::Left,
        click_count: 2,
        first_mouse: false,
    });
    cx.simulate_event(MouseUpEvent {
        position: at,
        modifiers: Modifiers::default(),
        button: MouseButton::Left,
        click_count: 2,
    });
}

fn three_sessions(
    cx: &mut TestAppContext,
) -> (
    gpui::Entity<crate::app::SiskinApp>,
    &mut gpui::VisualTestContext,
) {
    let (view, cx) = window(cx);
    click(cx, "new-session");
    wait_until(&view, cx, |app| app.sessions.len() == 2);
    click(cx, "new-session");
    wait_until(&view, cx, |app| app.sessions.len() == 3);
    wait_until(&view, cx, |app| app.active_index() == 2);
    (view, cx)
}

fn session_ids(
    view: &gpui::Entity<crate::app::SiskinApp>,
    cx: &mut gpui::VisualTestContext,
) -> Vec<u64> {
    read(view, cx, |app| {
        app.sessions.iter().map(|(_, session)| session.id).collect()
    })
}

#[gpui::test]
fn dragging_a_tab_reorders_sessions_and_keeps_the_active_session(cx: &mut TestAppContext) {
    let (view, cx) = three_sessions(cx);
    let ids = session_ids(&view, cx);
    // 按住第 0 个 tab，拖到第 2 个 tab 的位置再松开。
    redraw(cx);
    let from = cx.debug_bounds("session-0").unwrap().center();
    let to = cx.debug_bounds("session-2").unwrap().center();
    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::default());
    redraw(cx);
    assert_eq!(session_ids(&view, cx), vec![ids[1], ids[2], ids[0]]);
    // 活动会话是 ids[2]，移动前在下标 2，移动后跟到下标 1；
    // 松开那下合成的 click 必须被吞掉，不能顺手选了下标 2。
    assert_eq!(read(&view, cx, |app| app.active_index()), 1);
    assert_eq!(read(&view, cx, |app| app.session().id), ids[2]);
    // 标签点击顺序与 Sessions 顺序一致：点现在第 0 位的 tab 切到 ids[1]。
    click(cx, "session-0");
    assert_eq!(read(&view, cx, |app| app.active_index()), 0);
    assert_eq!(read(&view, cx, |app| app.session().id), ids[1]);
}

#[gpui::test]
fn double_clicking_a_tab_starts_an_inline_rename(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    redraw(cx);
    let center = cx.debug_bounds("session-0").unwrap().center();
    double_click(cx, center);
    redraw(cx);
    // 第一下单击只选中，第二下（click_count 2）进入内联重命名。
    assert!(read(&view, cx, |app| {
        app.tab_rename
            .as_ref()
            .is_some_and(|rename| rename.index == 0)
    }));
    cx.simulate_keystrokes("ctrl-a");
    cx.simulate_input("双击改名");
    cx.simulate_keystrokes("enter");
    assert_eq!(
        read(&view, cx, |app| app.session().display_title()),
        "双击改名"
    );
}

#[gpui::test]
fn renaming_the_active_tab_applies_immediately(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    redraw(cx);
    view.update(cx, |app, _| app.dispatch(Action::RenameSession));
    assert!(read(&view, cx, |app| app.tab_rename.is_some()));
    cx.simulate_keystrokes("ctrl-a");
    cx.simulate_input("工作");
    cx.simulate_keystrokes("enter");
    assert_eq!(
        read(&view, cx, |app| app.session().custom_title.clone()),
        Some("工作".to_string())
    );
    assert_eq!(read(&view, cx, |app| app.session().display_title()), "工作");
    assert!(read(&view, cx, |app| app.tab_rename.is_none()));
    // 清空提交 = 清除显式名，恢复目录回退。
    view.update(cx, |app, _| app.dispatch(Action::RenameSession));
    cx.simulate_keystrokes("ctrl-a");
    cx.simulate_keystrokes("backspace");
    cx.simulate_keystrokes("enter");
    assert_eq!(
        read(&view, cx, |app| app.session().custom_title.clone()),
        None
    );
}

#[gpui::test]
fn escape_cancels_a_rename_without_changing_the_title(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    redraw(cx);
    let before = read(&view, cx, |app| app.session().display_title());
    view.update(cx, |app, _| app.dispatch(Action::RenameSession));
    cx.simulate_keystrokes("ctrl-a");
    cx.simulate_input("改名");
    cx.simulate_keystrokes("escape");
    assert!(read(&view, cx, |app| app.tab_rename.is_none()));
    assert_eq!(
        read(&view, cx, |app| app.session().custom_title.clone()),
        None
    );
    assert_eq!(read(&view, cx, |app| app.session().display_title()), before);
}

#[gpui::test]
fn moving_a_split_tab_keeps_the_focused_pane(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    click(cx, "new-session");
    wait_until(&view, cx, |app| app.sessions.len() == 2);
    // 在活动标签（下标 1）里左右分屏：焦点跟到新叶。
    view.update(cx, |app, _| app.dispatch(Action::SplitVertical));
    redraw(cx);
    // 分屏叶不进标签栏：原来 2 个标签，分屏后还是 2 个。
    assert_eq!(read(&view, cx, |app| app.tab_indices().len()), 2);
    let focused = read(&view, cx, |app| app.active_pane);
    assert_eq!(read(&view, cx, |app| app.session().id), focused);
    // 把有分屏树的标签从下标 1 移到下标 0：移动只改向量顺序，
    // 会话 id 与焦点 pane 都不动，`session()` 仍是原焦点叶。
    view.update(cx, |app, _| {
        assert!(app.move_tab(1, 0));
    });
    assert_eq!(read(&view, cx, |app| app.active_index()), 0);
    assert_eq!(read(&view, cx, |app| app.active_pane), focused);
    assert_eq!(read(&view, cx, |app| app.session().id), focused);
    assert_eq!(
        read(&view, cx, |app| app
            .session_by_id(focused)
            .map(|session| session.id)),
        Some(focused)
    );
    assert_eq!(read(&view, cx, |app| app.tab_indices().len()), 2);
}
