// @Time    : 2026/11/4 20:18
// @Author  : fzf
// @FileName: tests.rs
// @Software: Siskin

use crate::{
    app::SiskinApp,
    dbusmenu::{MenuAction, MenuEvent},
    settings::{Page, SettingsSection, ShellKind},
    theme::ThemeId,
};
use gpui::{AppContext, Entity, Modifiers, TestAppContext, VisualTestContext, px, size};

fn redraw(cx: &mut VisualTestContext) {
    cx.update(|window, app| window.draw(app).clear(app));
}

fn click(cx: &mut VisualTestContext, id: &'static str) {
    redraw(cx);
    let viewport = cx.update(|window, _| window.viewport_size());
    for _ in 0..4 {
        let bounds = cx
            .debug_bounds(id)
            .unwrap_or_else(|| panic!("missing element {id}"));
        if bounds.top() >= px(0.) && bounds.bottom() <= viewport.height {
            break;
        }
        cx.simulate_event(gpui::ScrollEvent {
            position: gpui::point(viewport.width - px(64.), viewport.height / 2.),
            delta: gpui::ScrollDelta::Pixels(gpui::point(
                px(0.),
                viewport.height / 2. - bounds.center().y,
            )),
            ..Default::default()
        });
        redraw(cx);
    }
    let bounds = cx.debug_bounds(id).unwrap();
    cx.simulate_click(bounds.center(), Modifiers::default());
    redraw(cx);
}

fn window(cx: &mut TestAppContext) -> (Entity<SiskinApp>, &mut VisualTestContext) {
    let (view, cx) = cx.add_window_view(|window, cx| {
        let app = SiskinApp::new(cx);
        app.focus.focus(window, cx);
        app
    });
    cx.simulate_resize(size(px(1240.), px(800.)));
    (view, cx)
}

fn read<T>(
    view: &Entity<SiskinApp>,
    cx: &mut VisualTestContext,
    f: impl FnOnce(&SiskinApp) -> T,
) -> T {
    cx.update(|_, app| f(view.read(app)))
}

#[gpui::test]
fn startup_is_one_empty_session_without_dashboard_widgets(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    redraw(cx);
    assert_eq!(read(&view, cx, |app| app.sessions.len()), 1);
    assert_eq!(read(&view, cx, |app| app.command_count()), 0);
    for id in ["terminal-card", "session-0", "new-session", "nav-settings"] {
        assert!(cx.debug_bounds(id).is_some(), "missing {id}");
    }
    for id in [
        "activity-chart",
        "cpu-card",
        "memory-card",
        "terminal-status",
    ] {
        assert!(
            cx.debug_bounds(id).is_none(),
            "unexpected dashboard item {id}"
        );
    }
}

#[gpui::test]
fn terminal_input_and_empty_enter_behave_like_a_prompt(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    redraw(cx);
    let before = read(&view, cx, |app| app.line_count());
    cx.simulate_keystrokes("enter");
    assert_eq!(read(&view, cx, |app| app.line_count()), before);
    cx.simulate_input("whoami");
    assert_eq!(
        read(&view, cx, |app| app.session().terminal.input().to_string()),
        "whoami"
    );
    cx.simulate_keystrokes("enter");
    assert_eq!(read(&view, cx, |app| app.command_count()), 1);
    assert!(read(&view, cx, |app| app
        .session()
        .terminal
        .lines()
        .iter()
        .any(|line| line.plain() == "fzf")));
}

#[gpui::test]
fn sessions_add_select_close_and_never_close_the_last_session(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    click(cx, "new-session");
    assert_eq!(read(&view, cx, |app| app.sessions.len()), 2);
    assert_eq!(read(&view, cx, |app| app.active), 1);
    click(cx, "session-0");
    assert_eq!(read(&view, cx, |app| app.active), 0);
    click(cx, "close-session-1");
    assert_eq!(read(&view, cx, |app| app.sessions.len()), 1);
    click(cx, "close-session-0");
    assert_eq!(read(&view, cx, |app| app.sessions.len()), 1);
    assert!(cx.debug_bounds("terminal-input").is_some());
}

#[gpui::test]
fn inspector_is_optional_and_settings_stay_in_the_workspace(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    redraw(cx);
    assert!(cx.debug_bounds("session-inspector").is_none());
    click(cx, "toggle-inspector");
    assert!(read(&view, cx, |app| app.inspector_open));
    assert!(cx.debug_bounds("session-inspector").is_some());
    click(cx, "nav-settings");
    assert!(cx.debug_bounds("session-0").is_some());
    assert!(cx.debug_bounds("settings-page").is_some());
    assert!(cx.debug_bounds("session-inspector").is_none());
    click(cx, "settings-section-1");
    assert_eq!(
        read(&view, cx, |app| app.settings_section),
        SettingsSection::Typography
    );
    assert!(cx.debug_bounds("font-0").is_some());
    click(cx, "settings-done");
    assert!(cx.debug_bounds("session-inspector").is_some());
    click(cx, "close-inspector");
    assert!(!read(&view, cx, |app| app.inspector_open));
}

#[gpui::test]
fn theme_modes_fonts_and_shell_options_update_immediately(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    click(cx, "nav-settings");
    click(cx, "theme-mode-2");
    assert_eq!(read(&view, cx, |app| app.theme().id), ThemeId::Graphite);
    click(cx, "settings-section-2");
    click(cx, "shell-1");
    assert_eq!(read(&view, cx, |app| app.settings.shell), ShellKind::Bash);
    click(cx, "settings-section-1");
    let initial = read(&view, cx, |app| app.settings.font_index);
    click(cx, "font-up");
    assert_eq!(read(&view, cx, |app| app.settings.font_index), initial + 1);
    click(cx, "font-down");
    assert_eq!(read(&view, cx, |app| app.settings.font_index), initial);
    click(cx, "font-0");
    assert_eq!(read(&view, cx, |app| app.settings.font_family), 0);
}

#[gpui::test]
fn minimum_window_keeps_sidebar_and_workspace_visible(cx: &mut TestAppContext) {
    let (_, cx) = window(cx);
    cx.simulate_resize(size(px(960.), px(640.)));
    redraw(cx);
    for id in [
        "session-0",
        "new-session",
        "nav-settings",
        "terminal-input",
        "workspace-heading",
    ] {
        let bounds = cx.debug_bounds(id).unwrap();
        assert!(
            bounds.left() >= px(0.) && bounds.right() <= px(960.),
            "{id}: {bounds:?}"
        );
        assert!(
            bounds.top() >= px(0.) && bounds.bottom() <= px(640.),
            "{id}: {bounds:?}"
        );
    }
    let sidebar = cx.debug_bounds("session-list").unwrap();
    let workspace = cx.debug_bounds("workspace").unwrap();
    assert!(sidebar.right() <= workspace.left());
    assert!(workspace.size.width >= px(720.));
    click(cx, "nav-settings");
    click(cx, "settings-section-0");
    for id in [
        "settings-section-0",
        "settings-section-3",
        "theme-mode-0",
        "theme-mode-3",
    ] {
        let bounds = cx.debug_bounds(id).unwrap();
        assert!(bounds.right() <= px(960.), "{id}: {bounds:?}");
    }
}

#[gpui::test]
fn terminal_and_settings_typography_follow_the_selected_palette(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    click(cx, "nav-settings");
    click(cx, "theme-mode-2");
    assert!(read(&view, cx, |app| app.theme().dark));
    assert_eq!(read(&view, cx, |app| app.theme().ink), 0xe8ecf4);
    click(cx, "nav-terminal");
    assert!(read(&view, cx, |app| app.theme().dark));
    assert!(cx.debug_bounds("input-caret").is_some());
}

#[gpui::test]
fn custom_rgb_edits_validate_and_apply_to_terminal(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    click(cx, "nav-settings");
    click(cx, "theme-mode-3");
    click(cx, "color-background");
    cx.simulate_keystrokes("ctrl-a");
    cx.simulate_input("101827");
    cx.simulate_keystrokes("enter");
    assert_eq!(read(&view, cx, |app| app.theme().terminal), 0x101827);
    click(cx, "color-accent");
    cx.simulate_keystrokes("ctrl-a");
    cx.simulate_input("abc");
    cx.simulate_keystrokes("enter");
    assert!(read(&view, cx, |app| app
        .color_edit
        .as_ref()
        .unwrap()
        .error
        .is_some()));
    cx.simulate_keystrokes("ctrl-a");
    cx.simulate_input("61aaff");
    cx.simulate_keystrokes("enter");
    assert_eq!(read(&view, cx, |app| app.theme().accent), 0x61aaff);
}

#[gpui::test]
fn inspector_width_is_reserved_only_when_open_and_wide_enough(cx: &mut TestAppContext) {
    let (_, cx) = window(cx);
    let closed = cx.debug_bounds("workspace").unwrap().size.width;
    click(cx, "toggle-inspector");
    let open = cx.debug_bounds("workspace").unwrap().size.width;
    assert!(open < closed);
    cx.simulate_resize(size(px(900.), px(560.)));
    redraw(cx);
    assert!(cx.debug_bounds("session-inspector").is_none());
    assert!(cx.debug_bounds("workspace").unwrap().size.width >= px(680.));
}

#[gpui::test]
fn command_palette_and_navigation_shortcuts_work(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    redraw(cx);
    cx.simulate_keystrokes("ctrl-k");
    assert!(read(&view, cx, |app| app.palette_open()));
    cx.simulate_input("clear");
    cx.simulate_keystrokes("enter");
    assert!(!read(&view, cx, |app| app.palette_open()));
    cx.simulate_keystrokes("ctrl-,");
    assert_eq!(read(&view, cx, |app| app.page), Page::Settings);
    cx.simulate_keystrokes("ctrl-1");
    assert_eq!(read(&view, cx, |app| app.page), Page::Terminal);
}

#[gpui::test]
fn idle_menu_events_wake_the_view_without_input(cx: &mut TestAppContext) {
    let (sender, events) = async_channel::bounded(8);
    let view = cx.new(|cx| {
        let mut app = SiskinApp::new(cx);
        app.attach_menu_events(events, cx);
        app
    });
    sender
        .try_send(MenuEvent::Action(MenuAction::Settings))
        .unwrap();
    cx.run_until_parked();
    assert_eq!(view.read_with(cx, |app, _| app.page), Page::Settings);
    drop(sender);
    cx.run_until_parked();
}

#[gpui::test]
fn desktop_window_controls_are_not_duplicated_inside_the_app(cx: &mut TestAppContext) {
    let (_, cx) = window(cx);
    redraw(cx);
    for id in [
        "win-min",
        "win-max",
        "win-close",
        "win-fullscreen",
        "toggle-rail",
    ] {
        assert!(cx.debug_bounds(id).is_none());
    }
    assert!(cx.debug_bounds("settings-section-0").is_none());
    click(cx, "nav-settings");
    click(cx, "settings-section-3");
    assert!(cx.debug_bounds("settings-dbus-refresh").is_some());
    assert!(cx.debug_bounds("terminal-status").is_none());
}

#[gpui::test]
fn menu_shell_action_updates_active_session_and_displayed_tab(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    view.update(cx, |app, _| {
        app.set_page(Page::Settings);
        app.run_menu_action(MenuAction::SetShell(ShellKind::Bash))
    });
    assert_eq!(read(&view, cx, |app| app.session().kind), ShellKind::Bash);
    view.update(cx, |app, _| app.new_session());
    assert_eq!(read(&view, cx, |app| app.sessions.len()), 2);
    assert_eq!(read(&view, cx, |app| app.page), Page::Terminal);
}
