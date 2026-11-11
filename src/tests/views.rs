// @Time    : 2026/11/4 20:18
// @Author  : fzf
// @FileName: views.rs
// @Software: Siskin

use crate::{
    app::SiskinApp,
    domain::Action,
    io::dbusmenu::MenuEvent,
    settings::{Page, SettingsSection, ShellKind},
    theme::ThemeId,
};
use gpui::{AppContext, Entity, Modifiers, TestAppContext, VisualTestContext, px, size};

pub(super) fn redraw(cx: &mut VisualTestContext) {
    cx.update(|window, app| window.draw(app).clear(app));
}

pub(super) fn click(cx: &mut VisualTestContext, id: &'static str) {
    redraw(cx);
    let bounds = cx
        .debug_bounds(id)
        .unwrap_or_else(|| panic!("missing element {id}"));
    cx.simulate_click(bounds.center(), Modifiers::default());
    redraw(cx);
}

pub(super) fn window(cx: &mut TestAppContext) -> (Entity<SiskinApp>, &mut VisualTestContext) {
    let (view, cx) = cx.add_window_view(|window, cx| {
        let app = SiskinApp::new(cx);
        app.focus.focus(window, cx);
        app
    });
    cx.simulate_resize(size(px(1240.), px(800.)));
    (view, cx)
}

pub(super) fn read<T>(
    view: &Entity<SiskinApp>,
    cx: &mut VisualTestContext,
    f: impl FnOnce(&SiskinApp) -> T,
) -> T {
    cx.update(|_, app| f(view.read(app)))
}

/// 轮询到条件成立或超时。真实 PTY 异步派生并上报事件，后台循环与断言
/// 在赛跑，固定 sleep 既慢又仍会偶发失败，所以用轮询等稳态。
pub(super) fn wait_until(
    view: &Entity<SiskinApp>,
    cx: &mut VisualTestContext,
    predicate: impl Fn(&SiskinApp) -> bool,
) {
    // 不用 `read`：它收 `FnOnce`，轮询需要反复调用同一个闭包。
    for _ in 0..200 {
        let satisfied = cx.update(|_, app| predicate(view.read(app)));
        if satisfied {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[gpui::test]
fn startup_is_one_empty_session_without_dashboard_widgets(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    redraw(cx);
    assert_eq!(read(&view, cx, |app| app.sessions.len()), 1);
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

/// 没有真实 PTY 时（测试环境不派生进程），终端卡片仍然要渲染网格区域，
/// 只是不消费字符输入。
#[gpui::test]
fn terminal_grid_renders_without_a_pty(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    redraw(cx);
    assert!(cx.debug_bounds("terminal-grid").is_some());
    cx.simulate_keystrokes("enter");
    assert!(read(&view, cx, |app| !app.session().frame.rows.is_empty()));
}

#[gpui::test]
fn sessions_add_and_switch_tabs(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    click(cx, "new-session");
    wait_until(&view, cx, |app| app.sessions.len() == 2);
    assert_eq!(read(&view, cx, |app| app.sessions.len()), 2);
    // 等真实 PTY 的首屏事件回流后再操作，避免与事件循环抢状态。
    wait_until(&view, cx, |app| app.active_index() == 1);
    // 点第 0 个 tab 应当切回它。确认框的分支由仓储层测试覆盖，
    // 这里只验证 tab 的点击链路与 id 稳定。
    click(cx, "session-0");
    assert_eq!(read(&view, cx, |app| app.active_index()), 0);
    assert_eq!(
        read(&view, cx, |app| app.sessions.len()),
        2,
        "切会话不该增减会话数"
    );
}

#[gpui::test]
fn inspector_is_optional_and_settings_stay_in_the_workspace(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    redraw(cx);
    assert!(cx.debug_bounds("session-inspector").is_none());
    cx.simulate_keystrokes("ctrl-shift-i");
    assert!(read(&view, cx, |app| app.inspector_open));
    assert!(cx.debug_bounds("session-inspector").is_some());
    click(cx, "nav-settings");
    assert!(cx.debug_bounds("session-0").is_none());
    assert!(cx.debug_bounds("settings-page").is_some());
    assert!(cx.debug_bounds("session-inspector").is_none());
    click(cx, "settings-section-1");
    assert_eq!(
        read(&view, cx, |app| app.settings_section),
        SettingsSection::Typography
    );
    assert!(cx.debug_bounds("toggle-font-picker").is_some());
    assert!(cx.debug_bounds("font-popup").is_none());
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
    click(cx, "toggle-font-picker");
    click(cx, "font-0");
    assert_eq!(read(&view, cx, |app| app.settings.font_family), 0);
}

#[gpui::test]
fn minimum_window_keeps_tabs_and_workspace_visible(cx: &mut TestAppContext) {
    let (_, cx) = window(cx);
    cx.simulate_resize(size(px(960.), px(640.)));
    redraw(cx);
    for id in ["session-0", "new-session", "nav-settings", "terminal-grid"] {
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
    // 没有侧栏后，终端区应该铺满整窗宽度，最小窗口下也保住可视列数。
    let workspace = cx.debug_bounds("workspace").unwrap();
    assert!(workspace.left() == px(0.));
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
    click(cx, "settings-done");
    assert!(read(&view, cx, |app| app.theme().dark));
    assert!(cx.debug_bounds("terminal-grid").is_some());
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
    cx.simulate_keystrokes("ctrl-shift-i");
    redraw(cx);
    let open = cx.debug_bounds("workspace").unwrap().size.width;
    assert!(open < closed);
    cx.simulate_resize(size(px(900.), px(560.)));
    redraw(cx);
    assert!(cx.debug_bounds("session-inspector").is_none());
    // 900 < 1100 阈值，检视面板不占位；侧栏已移除，终端区铺满整窗。
    assert_eq!(cx.debug_bounds("workspace").unwrap().size.width, px(900.));
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
        .try_send(MenuEvent::Action(Action::OpenSettings))
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
fn a_failed_save_is_visible_on_the_settings_page(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    click(cx, "nav-settings");
    assert!(cx.debug_bounds("settings-save-error").is_none());
    view.update(cx, |app, _| {
        app.save_error = Some("磁盘只读".to_string());
    });
    redraw(cx);
    assert!(cx.debug_bounds("settings-save-error").is_some());
}

#[gpui::test]
fn menu_shell_action_updates_new_sessions_and_displayed_tab(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    view.update(cx, |app, _| {
        app.set_page(Page::Settings);
        app.dispatch(Action::SetShell(ShellKind::Bash))
    });
    // 已运行的会话保留自己的进程，改的是新建会话的默认值。
    assert_eq!(read(&view, cx, |app| app.settings.shell), ShellKind::Bash);
    view.update(cx, |app, _| app.new_session());
    assert_eq!(read(&view, cx, |app| app.sessions.len()), 2);
    assert_eq!(read(&view, cx, |app| app.session().kind), ShellKind::Bash);
    assert_eq!(read(&view, cx, |app| app.page), Page::Terminal);
}

#[gpui::test]
fn palette_theme_actions_select_the_new_presets(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    for theme in [ThemeId::CatppuccinMocha, ThemeId::TokyoNight, ThemeId::Moss] {
        view.update(cx, |app, _| {
            app.dispatch(Action::SetTheme(theme));
        });
        assert_eq!(read(&view, cx, |app| app.theme().id), theme);
        assert_eq!(read(&view, cx, |app| app.theme().dark), theme.is_dark());
    }
    // 点选之后再切模式，预设固定即解除，回到按模式解析。
    view.update(cx, |app, _| {
        app.dispatch(Action::CycleThemeMode);
    });
    assert_eq!(read(&view, cx, |app| app.theme().id), ThemeId::Graphite);
}
