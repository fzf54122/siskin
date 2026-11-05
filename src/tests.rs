// @Time    : 2026/11/4 20:18
// @Author  : fzf
// @FileName: tests.rs
// @Software: Siskin

//! UI-level checks: real focus, real clicks, real keystrokes.

use crate::app::SiskinApp;
use gpui::{Entity, Modifiers, TestAppContext, VisualTestContext, px, size};

fn redraw(cx: &mut VisualTestContext) {
    cx.update(|window, app| window.draw(app).clear(app));
}

fn click(cx: &mut VisualTestContext, id: &'static str) {
    redraw(cx);
    let bounds = cx
        .debug_bounds(id)
        .unwrap_or_else(|| panic!("missing element {id}"));
    cx.simulate_click(bounds.center(), Modifiers::default());
    redraw(cx);
}

fn read<T>(
    view: &Entity<SiskinApp>,
    cx: &mut VisualTestContext,
    f: impl FnOnce(&SiskinApp) -> T,
) -> T {
    cx.update(|_, app| f(view.read(app)))
}

#[gpui::test]
fn typing_and_enter_flow_through_the_focused_window(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|window, cx| {
        let app = SiskinApp::new(cx);
        app.focus.focus(window, cx);
        app
    });
    cx.simulate_resize(size(px(1240.), px(800.)));
    redraw(cx);

    cx.simulate_input("whoami");
    assert_eq!(
        read(&view, cx, |app| app.session().terminal.input().to_string()),
        "whoami"
    );

    cx.simulate_keystrokes("enter");
    let lines = read(&view, cx, |app| {
        app.session()
            .terminal
            .lines()
            .iter()
            .map(|line| line.plain())
            .collect::<Vec<_>>()
            .join("\n")
    });
    assert!(lines.contains("whoami"));
    assert!(lines.contains("fzf"));
    assert!(read(&view, cx, |app| app
        .session()
        .terminal
        .input()
        .is_empty()));
}

#[gpui::test]
fn sessions_and_palette_are_reachable_by_mouse(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|window, cx| {
        let app = SiskinApp::new(cx);
        app.focus.focus(window, cx);
        app
    });
    cx.simulate_resize(size(px(1240.), px(800.)));

    click(cx, "session-1");
    assert_eq!(read(&view, cx, |app| app.active), 1);

    click(cx, "new-session");
    assert_eq!(read(&view, cx, |app| app.sessions.len()), 4);
    assert_eq!(read(&view, cx, |app| app.active), 3);

    click(cx, "toggle-rail");
    assert!(!read(&view, cx, |app| app.rail));
    click(cx, "toggle-rail");
    assert!(read(&view, cx, |app| app.rail));
}

#[gpui::test]
fn keyboard_shortcuts_open_and_close_the_palette(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|window, cx| {
        let app = SiskinApp::new(cx);
        app.focus.focus(window, cx);
        app
    });
    cx.simulate_resize(size(px(1240.), px(800.)));
    redraw(cx);

    cx.simulate_keystrokes("ctrl-k");
    assert!(read(&view, cx, |app| app.palette_open()));
    redraw(cx);
    cx.simulate_input("clear");
    redraw(cx);
    cx.simulate_keystrokes("enter");
    assert!(!read(&view, cx, |app| app.palette_open()));

    cx.simulate_keystrokes("ctrl-k");
    assert!(read(&view, cx, |app| app.palette_open()));
    cx.simulate_keystrokes("escape");
    assert!(!read(&view, cx, |app| app.palette_open()));
}

#[gpui::test]
fn rail_buttons_and_scroll_state_stay_in_sync(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|window, cx| {
        let app = SiskinApp::new(cx);
        app.focus.focus(window, cx);
        app
    });
    cx.simulate_resize(size(px(1240.), px(800.)));

    let commands = read(&view, cx, |app| app.command_count());
    click(cx, "rail-help");
    assert!(read(&view, cx, |app| app.line_count()) > 5);
    assert_eq!(read(&view, cx, |app| app.command_count()), commands + 1);

    click(cx, "rail-clear");
    assert_eq!(read(&view, cx, |app| app.line_count()), 0);
    assert_eq!(read(&view, cx, |app| app.command_count()), commands + 1);
}

#[gpui::test]
fn minimum_window_keeps_input_and_sidebar_visible(cx: &mut TestAppContext) {
    let (_, cx) = cx.add_window_view(|window, cx| {
        let app = SiskinApp::new(cx);
        app.focus.focus(window, cx);
        app
    });
    cx.simulate_resize(size(px(900.), px(560.)));
    redraw(cx);

    for id in ["session-0", "new-session", "toggle-rail"] {
        let bounds = cx
            .debug_bounds(id)
            .unwrap_or_else(|| panic!("{id} missing"));
        assert!(bounds.left() >= px(0.));
        assert!(bounds.right() <= px(900.), "{id} overflows: {bounds:?}");
        assert!(bounds.bottom() <= px(560.), "{id} overflows: {bounds:?}");
    }
}
