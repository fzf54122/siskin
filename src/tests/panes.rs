// @Time    : 2026/11/11
// @Author  : fzf
// @FileName: panes.rs
// @Software: Siskin

//! 分屏集成测试：用 GPUI 测试模式走真实渲染与按键链路。
//! 分裂是同步的（会话先进入仓储再派生 PTY），断言不需要等 PTY；
//! 派生失败也不影响布局断言，会话会以 Failed 状态留在树里。

use super::views::{read, redraw, window};
use crate::domain::Action;
use gpui::{TestAppContext, px};

#[gpui::test]
fn splitting_creates_a_second_pane_without_a_second_tab(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    redraw(cx);
    cx.simulate_keystrokes("ctrl-shift-r");
    redraw(cx);
    // 左右分屏：会话数 2，但标签仍是 1 个，分屏叶不进标签栏。
    assert_eq!(read(&view, cx, |app| app.sessions.len()), 2);
    assert_eq!(read(&view, cx, |app| app.tab_indices().len()), 1);
    assert!(cx.debug_bounds("session-1").is_none());
    let (first, second, active) = read(&view, cx, |app| {
        let ids = app.active_tab_tree().leaf_ids();
        (ids[0], ids[1], app.active_pane)
    });
    // 新 pane 在右方，焦点跟过去；`session()` 就是焦点 pane 的会话。
    assert_eq!(active, second);
    assert_eq!(read(&view, cx, |app| app.session().id), second);
    assert_ne!(first, second);
    // 两叶左右并排：同高同顶，宽各约一半（1240 宽减 4px 分隔线）。
    let bounds = read(&view, cx, |app| app.pane_bounds.clone());
    assert_eq!(bounds.len(), 2);
    let left = bounds[&first];
    let right = bounds[&second];
    assert_eq!(left.origin.y, right.origin.y);
    assert_eq!(left.size.height, right.size.height);
    assert!((left.size.width.as_f32() - 618.).abs() < 1.);
    assert!((right.size.width.as_f32() - 618.).abs() < 1.);
    assert_eq!(right.origin.x, left.origin.x + left.size.width + px(4.));
    assert!(cx.debug_bounds("pane-divider").is_some());
    // 各叶按自己的区域算行列，不再是整窗尺寸。
    let (left_columns, right_columns) = read(&view, cx, |app| {
        (
            app.session_by_id(first).map(|session| session.columns),
            app.session_by_id(second).map(|session| session.columns),
        )
    });
    assert_eq!(left_columns, right_columns);
    let full_columns = read(&view, cx, |app| app.metrics.columns(1240.));
    assert!(left_columns.unwrap() < full_columns);
}

#[gpui::test]
fn focus_cycles_between_panes_and_closing_keeps_the_other(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    view.update(cx, |app, _| app.dispatch(Action::SplitVertical));
    view.update(cx, |app, _| app.dispatch(Action::SplitHorizontal));
    redraw(cx);
    let ids = read(&view, cx, |app| app.active_tab_tree().leaf_ids());
    assert_eq!(ids.len(), 3);
    assert_eq!(read(&view, cx, |app| app.active_pane), ids[2]);
    // 焦点循环：2 → 0 → 2，`session()` 跟着焦点走。
    view.update(cx, |app, _| app.dispatch(Action::FocusNextPane));
    assert_eq!(read(&view, cx, |app| app.active_pane), ids[0]);
    assert_eq!(read(&view, cx, |app| app.session().id), ids[0]);
    view.update(cx, |app, _| app.dispatch(Action::FocusPrevPane));
    assert_eq!(read(&view, cx, |app| app.active_pane), ids[2]);
    // 关掉焦点 pane（叶 2）：剩下 0、1，焦点落到相邻叶 0。
    view.update(cx, |app, _| app.dispatch(Action::ClosePane));
    redraw(cx);
    assert_eq!(read(&view, cx, |app| app.sessions.len()), 2);
    assert_eq!(
        read(&view, cx, |app| app.active_tab_tree().leaf_ids()),
        vec![ids[0], ids[1]]
    );
    assert_eq!(read(&view, cx, |app| app.active_pane), ids[0]);
    assert_eq!(read(&view, cx, |app| app.session().id), ids[0]);
    // 剩下的叶回到整窗宽度：行列按单叶重算。
    let full_columns = read(&view, cx, |app| app.metrics.columns(1240.));
    view.update(cx, |app, _| app.dispatch(Action::ClosePane));
    redraw(cx);
    assert_eq!(read(&view, cx, |app| app.sessions.len()), 1);
    assert_eq!(read(&view, cx, |app| app.session().columns), full_columns);
}

#[gpui::test]
fn divider_drag_changes_the_split_ratio(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    view.update(cx, |app, _| app.dispatch(Action::SplitVertical));
    redraw(cx);
    let second = read(&view, cx, |app| app.active_tab_tree().leaf_ids()[1]);
    view.update(cx, |app, _| {
        let rect = app.pane_bounds[&second];
        // 分隔线中心：右叶左缘往左 2px，沿 x 向右拖 120px。
        let start = gpui::point(rect.origin.x - px(2.), rect.origin.y + px(10.));
        app.start_pane_drag(second, start);
        app.update_pane_drag(gpui::point(start.x + px(120.), start.y));
        app.end_pane_drag();
    });
    let ratio = read(&view, cx, |app| {
        app.active_tab_tree().split_info(second).map(|info| info.1)
    });
    assert!(ratio.unwrap() > 0.5, "右拖应让左叶占比变大：{ratio:?}");
    redraw(cx);
    let (first, second) = read(&view, cx, |app| {
        let ids = app.active_tab_tree().leaf_ids();
        (ids[0], ids[1])
    });
    let bounds = read(&view, cx, |app| app.pane_bounds.clone());
    let left_width = bounds[&first].size.width.as_f32();
    let right_width = bounds[&second].size.width.as_f32();
    assert!(left_width > right_width + 100.);
}

#[gpui::test]
fn closing_the_last_pane_closes_the_tab(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    view.update(cx, |app, _| app.dispatch(Action::SplitVertical));
    redraw(cx);
    assert_eq!(read(&view, cx, |app| app.sessions.len()), 2);
    view.update(cx, |app, _| app.dispatch(Action::ClosePane));
    assert_eq!(read(&view, cx, |app| app.sessions.len()), 1);
    assert!(!read(&view, cx, |app| app.request_quit));
    // 只剩一个叶时再关 pane，等同于关标签：最后一个标签请求退出。
    view.update(cx, |app, _| app.dispatch(Action::ClosePane));
    assert!(read(&view, cx, |app| app.request_quit));
}
