// @Time    : 2026/11/5 18:40
// @Author  : fzf
// @FileName: keys.rs
// @Software: Siskin

//! 按键责任链。这里直接构造 `Keystroke` 调 `handle_key`，不经过窗口，
//! 所以每条分支都能单独断言「谁消费了按键」。真正写入 PTY 的字节编码
//! 在 `io::keys` 的单元测试里按表格覆盖。

use gpui::{AppContext as _, Keystroke};

use crate::app::{Handled, SiskinApp};
use crate::settings::Page;

fn app(cx: &mut gpui::TestAppContext) -> gpui::Entity<SiskinApp> {
    cx.new(SiskinApp::new)
}

fn press(text: &str) -> Keystroke {
    Keystroke::parse(text).expect("测试里的按键写法必须合法")
}

/// 返回是否被消费，顺便统一断言 `Handled` 只有两种取值。
fn send(app: &mut SiskinApp, text: &str) -> Option<Handled> {
    app.handle_key(&press(text))
}

#[gpui::test]
fn palette_filters_and_runs_actions(cx: &mut gpui::TestAppContext) {
    let app = app(cx);
    app.update(cx, |app, _| {
        assert!(matches!(send(app, "ctrl-k"), Some(Handled::Redraw)));
        assert!(app.palette_open());
        for ch in "关闭".chars() {
            app.palette_type(ch);
        }
        let labels: Vec<_> = app.palette_entries().iter().map(|e| e.label).collect();
        assert!(labels.contains(&"关闭当前会话"));
        send(app, "enter");
        assert!(!app.palette_open());
    });
}

#[gpui::test]
fn palette_escape_and_empty_query_behave(cx: &mut gpui::TestAppContext) {
    let app = app(cx);
    app.update(cx, |app, _| {
        send(app, "ctrl-k");
        assert!(app.palette_open());
        send(app, "escape");
        assert!(!app.palette_open());

        app.open_palette();
        for ch in "xyz".chars() {
            app.palette_type(ch);
        }
        assert!(app.palette_entries().is_empty());
        send(app, "enter");
        assert!(!app.palette_open());
    });
}

/// 快捷键与页面状态的交叉矩阵。新增快捷键时这里补一行，防止某条分支
/// 在设置页或面板打开时被前面的环吞掉。
#[gpui::test]
fn shortcut_matrix_routes_each_combination_once(cx: &mut gpui::TestAppContext) {
    let app = app(cx);
    app.update(cx, |app, _| {
        let cases = [
            (Page::Terminal, "ctrl-,", Page::Settings),
            (Page::Settings, "ctrl-,", Page::Settings),
            (Page::Settings, "ctrl-1", Page::Terminal),
            (Page::Terminal, "ctrl-1", Page::Terminal),
        ];
        for (before, text, expected) in cases {
            app.set_page(before);
            assert!(send(app, text).is_some(), "{text} 应被快捷键环消费");
            assert_eq!(app.page, expected, "{before:?} + {text}");
        }
    });
}

/// 没有运行中的 PTY 时：字符不吞，Enter 用来关闭/重启这个会话。
#[gpui::test]
fn keys_behave_sensibly_without_a_live_pty(cx: &mut gpui::TestAppContext) {
    let app = app(cx);
    app.update(cx, |app, _| {
        assert!(send(app, "a").is_none());
        // Enter 是「关闭已退出的会话」，只剩一个时等于重启，会话数不变。
        assert!(matches!(send(app, "enter"), Some(Handled::Redraw)));
        assert_eq!(app.sessions.len(), 1);
        // 未绑定的 Ctrl 组合同样交还框架。
        assert!(send(app, "ctrl-shift-x").is_none());
    });
}

#[gpui::test]
fn theme_and_font_shortcuts_apply_immediately(cx: &mut gpui::TestAppContext) {
    let app = app(cx);
    app.update(cx, |app, _| {
        let before = app.settings.font_index;
        assert!(matches!(send(app, "ctrl-="), Some(Handled::Redraw)));
        assert_eq!(app.settings.font_index, before + 1);
        send(app, "ctrl--");
        assert_eq!(app.settings.font_index, before);

        let mode = app.settings.mode;
        send(app, "ctrl-shift-t");
        assert_ne!(app.settings.mode, mode);
    });
}

#[gpui::test]
fn search_opens_types_and_closes(cx: &mut gpui::TestAppContext) {
    let app = app(cx);
    app.update(cx, |app, _| {
        assert!(app.search.is_none());
        assert!(matches!(send(app, "ctrl-shift-f"), Some(Handled::Redraw)));
        assert!(app.search.is_some());
        // 真实按键同时带键名与字符；只用键名时平台不会给出可插入文本。
        for ch in ["e", "r", "r"] {
            send(app, ch);
        }
        assert_eq!(
            app.search.as_ref().map(|state| state.query.as_str()),
            Some("err")
        );
        assert!(matches!(send(app, "backspace"), Some(Handled::Redraw)));
        assert_eq!(
            app.search.as_ref().map(|state| state.query.as_str()),
            Some("er")
        );
        assert!(matches!(send(app, "escape"), Some(Handled::Redraw)));
        assert!(app.search.is_none());
    });
}
