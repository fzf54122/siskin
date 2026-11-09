// @Time    : 2026/11/5 17:42
// @Author  : fzf
// @FileName: layout.rs
// @Software: Siskin

use super::views::{click, read, redraw, window};
use crate::{
    app::{CloseRequest, Session},
    domain::{Action, SessionState},
    settings::{Page, ShellKind, ThemeMode},
};
use gpui::{AppContext, EntityInputHandler, Keystroke, TestAppContext, px};

#[gpui::test]
fn approved_layout_keeps_a_single_row_rail_and_flat_forms(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    redraw(cx);
    assert_eq!(
        cx.debug_bounds("workspace-heading").unwrap().size.height,
        px(42.)
    );
    assert_eq!(cx.debug_bounds("session-0").unwrap().size.height, px(40.));
    assert_eq!(cx.debug_bounds("workspace").unwrap().left(), px(224.));
    click(cx, "nav-settings");
    assert!(cx.debug_bounds("session-list").is_none());
    assert!(cx.debug_bounds("workspace-heading").is_none());
    assert!(cx.debug_bounds("theme-preview").is_none());
    let bounds: Vec<_> = ["color-accent", "color-background", "color-foreground"]
        .map(|id| cx.debug_bounds(id).unwrap())
        .into();
    assert!(
        bounds
            .iter()
            .all(|item| item.size.width == px(240.) && item.right() == bounds[0].right())
    );
    assert_eq!(read(&view, cx, |app| app.theme().ink), 0x3b3d42);
}

#[gpui::test]
fn settings_arrows_do_not_mutate_preferences_or_send_terminal_input(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    click(cx, "nav-settings");
    let before = read(&view, cx, |app| {
        (app.settings.mode, app.settings.font_index)
    });
    cx.simulate_keystrokes("up down left right tab enter");
    assert_eq!(
        read(&view, cx, |app| (
            app.settings.mode,
            app.settings.font_index
        )),
        before
    );
    assert_eq!(read(&view, cx, |app| app.page), Page::Settings);
    cx.simulate_keystrokes("escape");
    assert_eq!(read(&view, cx, |app| app.page), Page::Terminal);
}

#[gpui::test]
fn font_picker_filters_accepts_keyboard_and_returns_to_terminal(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    view.update(cx, |app, _| {
        app.mono_fonts = vec!["Noto Sans Mono".into(), "DejaVu Sans Mono".into()];
    });
    click(cx, "nav-settings");
    click(cx, "settings-section-1");
    click(cx, "toggle-font-picker");
    cx.simulate_input("Deja");
    redraw(cx);
    assert!(cx.debug_bounds("font-0").is_none());
    assert!(cx.debug_bounds("font-1").is_some());
    cx.simulate_keystrokes("enter");
    assert_eq!(read(&view, cx, |app| app.settings.font_family), 1);
    assert!(read(&view, cx, |app| app.font_picker.is_none()));
    click(cx, "settings-done");
    assert_eq!(read(&view, cx, |app| app.page), Page::Terminal);
}

#[gpui::test]
fn color_editor_supports_selection_paste_validation_and_cancellation(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    click(cx, "nav-settings");
    click(cx, "color-background");
    cx.simulate_keystrokes("ctrl-a");
    cx.update(|_, app| app.write_to_clipboard(gpui::ClipboardItem::new_string("#ABCDEF".into())));
    cx.simulate_keystrokes("ctrl-v");
    click(cx, "color-background-apply");
    assert_eq!(read(&view, cx, |app| app.theme().terminal), 0xabcdef);
    click(cx, "color-foreground");
    cx.simulate_keystrokes("ctrl-a");
    cx.simulate_input("invalid");
    cx.simulate_keystrokes("enter");
    assert!(read(&view, cx, |app| app
        .color_edit
        .as_ref()
        .unwrap()
        .error
        .is_some()));
    cx.simulate_keystrokes("escape");
    assert_eq!(read(&view, cx, |app| app.theme().ink), 0x3b3d42);
}

#[gpui::test]
fn color_preedit_replaces_utf16_range_without_writing_to_the_pty(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    click(cx, "nav-settings");
    click(cx, "color-background");
    view.update_in(cx, |app, window, cx| {
        app.replace_and_mark_text_in_range(None, "abc", None, window, cx);
        app.replace_text_in_range(None, "123456", window, cx);
        assert_eq!(app.color_edit.as_ref().unwrap().draft.text, "123456");
        assert!(app.session().marked.is_none());
    });
    cx.simulate_keystrokes("enter");
    assert_eq!(read(&view, cx, |app| app.theme().terminal), 0x123456);
}

#[gpui::test]
fn close_confirmation_uses_stable_session_ids_and_cancel_is_non_destructive(
    cx: &mut TestAppContext,
) {
    let (view, cx) = window(cx);
    view.update(cx, |app, _| {
        for id in [10, 20] {
            app.sessions
                .add(Session::starting(id, "session", ShellKind::Sh));
        }
        app.close_request = Some(CloseRequest::Session(20));
    });
    redraw(cx);
    cx.simulate_keystrokes("enter");
    assert_eq!(read(&view, cx, |app| app.sessions.len()), 3);
    click(cx, "cancel-close");
    assert_eq!(read(&view, cx, |app| app.sessions.len()), 3);
    view.update(cx, |app, _| {
        app.close_request = Some(CloseRequest::Session(20));
        app.close_session(0);
    });
    click(cx, "confirm-close");
    assert_eq!(read(&view, cx, |app| app.sessions.len()), 1);
    assert_eq!(read(&view, cx, |app| app.session().id), 10);
    assert!(!read(&view, cx, |app| app.request_quit));
}

#[gpui::test]
fn closing_the_last_session_requests_exit_without_restarting(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    view.update(cx, |app, _| {
        app.session_mut().state = SessionState::Failed("not running".into());
        app.close_session(0);
        assert!(app.request_quit);
        assert!(matches!(app.session().state, SessionState::Failed(_)));
        assert!(app.session().pty().is_none());
    });
}

#[gpui::test]
fn settings_save_state_and_invalid_file_preservation_are_truthful(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("settings.json");
    let app = cx.new(crate::app::SiskinApp::new);
    app.update(cx, |app, _| {
        app.load_preferences(path.clone());
        app.dispatch(Action::CycleThemeMode);
        assert_eq!(app.save_label(), "保存中…");
        app.dirty_since = Some(std::time::Instant::now() - std::time::Duration::from_secs(1));
        assert!(app.flush_preferences_if_due());
        assert_eq!(app.save_label(), "已保存");
    });
    assert!(path.exists());
    std::fs::write(&path, "{broken").unwrap();
    app.update(cx, |app, _| {
        app.load_preferences(path.clone());
        app.set_theme_mode(ThemeMode::Dark);
        app.flush_preferences();
        assert_eq!(app.save_label(), "保存失败");
    });
    assert_eq!(std::fs::read_to_string(path).unwrap(), "{broken");
}

#[gpui::test]
fn search_does_not_capture_keys_after_switching_to_settings(cx: &mut TestAppContext) {
    let app = cx.new(crate::app::SiskinApp::new);
    app.update(cx, |app, _| {
        app.open_search();
        app.set_page(Page::Settings);
        assert!(app.search.is_none());
        let before = app.settings.font_index;
        app.handle_key(&Keystroke::parse("up").unwrap());
        assert_eq!(app.settings.font_index, before);
    });
}
