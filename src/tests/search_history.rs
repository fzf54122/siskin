// @Time    : 2026/11/11
// @Author  : fzf
// @FileName: search_history.rs
// @Software: Siskin

//! P3 历史搜索的真实 PTY 集成测试：输出超过一屏后，历史里的词能找到、
//! 步进能滚到该行且高亮行一致；无效正则不崩溃且 label 有错误；
//! 10k 行单次全量搜索在 2 秒内完成。不依赖显示器，CI 里也能跑。

use std::time::{Duration, Instant};

use gpui::{AppContext as _, Keystroke};

use crate::app::SiskinApp;
use crate::domain::session::SessionSpec;
use crate::io::ansi::AnsiColors;
use crate::io::pty::Pty;
use crate::io::vt::{ScreenSize, VtEvent};
use crate::theme::PAPER;

use super::pty::{spawn, wait_for_output};

fn frame_text(pty: &Pty) -> String {
    pty.frame(&AnsiColors::from_palette(&PAPER))
        .rows
        .iter()
        .flat_map(|row| row.cells.iter())
        .map(|cell| cell.text.as_str())
        .collect()
}

fn spawn_with_history(
    spec: SessionSpec,
    history: usize,
) -> (Pty, async_channel::Receiver<(u64, VtEvent)>) {
    let (sender, receiver) = async_channel::unbounded();
    let pty = Pty::spawn(
        &spec.shell,
        spec.working_directory,
        ScreenSize::new(40, 10, history),
        sender,
        0,
        0,
        &[("TERM".into(), "xterm-256color".into())],
    )
    .expect("测试环境必须能派生 PTY");
    (pty, receiver)
}

fn seq_spec(command: &str) -> SessionSpec {
    SessionSpec::command(
        "test",
        std::path::PathBuf::from("/bin/sh"),
        vec!["-c".into(), command.into()],
    )
}

#[test]
fn history_search_finds_a_word_scrolled_off_screen() {
    let (mut pty, _events) = spawn(seq_spec("seq 1 500; sleep 5"));
    assert!(wait_for_output(&pty, "500"), "未读到 seq 尾部");
    let spans = pty
        .search_history("350", false, false)
        .expect("字面查询不应报错");
    assert_eq!(spans.len(), 1, "350 在输出里只出现一次");
    let span = spans[0];
    assert!(span.line < 0, "350 行早已滚出可视区，应为历史行");
    assert!(!frame_text(&pty).contains("350"), "可视帧不应含 350");
    // 按 step_match 的换算滚到该行：行号 + 偏移 = 可见行。
    let delta = 10 - 1 - span.line;
    pty.scroll(delta);
    let frame = pty.frame(&AnsiColors::from_palette(&PAPER));
    assert_eq!(frame.display_offset, delta as usize, "应正好滚到目标行");
    let row = (span.line + frame.display_offset as i32) as usize;
    assert_eq!(row, 9, "350 行应落在可见区最后一行");
    let text: String = frame.rows[row]
        .cells
        .iter()
        .map(|cell| cell.text.as_str())
        .collect();
    assert!(text.contains("350"), "滚到后该行内容应是 350");
}

#[test]
fn an_invalid_regex_from_the_pty_reports_an_error() {
    let (pty, _events) = spawn(seq_spec("sleep 2"));
    let error = pty.search_history("[", false, true);
    assert!(error.is_err(), "无效正则必须返回错误而不是崩溃");
}

#[test]
fn searching_ten_thousand_lines_finishes_within_two_seconds() {
    let (pty, _events) = spawn_with_history(seq_spec("seq 1 10000; sleep 5"), 10_000);
    assert!(wait_for_output(&pty, "10000"), "未读到 seq 尾部");
    let started = Instant::now();
    let spans = pty
        .search_history("5000", false, false)
        .expect("字面查询不应报错");
    let elapsed = started.elapsed();
    assert_eq!(spans.len(), 1, "5000 在输出里只出现一次");
    assert!(spans[0].line < 0, "5000 行应在历史区");
    assert!(
        elapsed < Duration::from_secs(2),
        "10k 行全量搜索耗时 {elapsed:?}，超过 2 秒上限"
    );
}

fn session_text(app: &SiskinApp) -> String {
    app.session()
        .frame
        .rows
        .iter()
        .flat_map(|row| row.cells.iter())
        .map(|cell| cell.text.as_str())
        .collect()
}

fn search(app: &SiskinApp) -> &crate::app::SearchState {
    app.search.as_ref().expect("搜索条应打开")
}

/// 排空事件直到会话帧里出现 `needle`（15 秒超时即失败）。
fn wait_for_session_text(app: &mut SiskinApp, needle: &str) {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        app.drain_events();
        if session_text(app).contains(needle) {
            return;
        }
        if Instant::now() > deadline {
            panic!("15 秒内未读到 {needle}");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// 应用层闭环：新输出到达后 `drain_events` 自动重算搜索，步进滚到
/// 历史行后当前匹配与高亮行一致。
#[gpui::test]
fn step_match_scrolls_a_history_match_into_view(cx: &mut gpui::TestAppContext) {
    let app = cx.new(SiskinApp::new);
    app.update(cx, |app, _| {
        // 先打字、后派生：打字时还没有 PTY，匹配必为空；派生后输出到达，
        // drain 必须自动重算出 350。直接在派生后断言“输出未到”会竞态：
        // 搜索读的是实时 PTY 网格，seq 可能在打字前就已进网格。
        app.open_search();
        for ch in "350".chars() {
            app.search_type(ch);
        }
        assert!(search(app).matches.is_empty(), "无 PTY 时不应有匹配");
        let (sender, receiver) = async_channel::unbounded();
        app.event_sender = Some(sender.clone());
        app.event_receiver = Some(receiver);
        app.session_mut()
            .spawn_spec(&seq_spec("seq 1 500; sleep 5"), sender, 0)
            .expect("测试环境必须能派生 PTY");
        wait_for_session_text(app, "500");
        assert_eq!(
            search(app).matches.len(),
            1,
            "新输出到达后 drain 应自动重算出 350"
        );
        let span = search(app).current_span().expect("应有当前匹配");
        assert!(span.line < 0, "350 行应在历史区");
        app.step_match(true);
        let offset = app.session().frame.display_offset;
        assert!(offset > 0, "步进应滚到历史行");
        let rows = app.session().frame.rows.len();
        let visible = search(app)
            .current_visible_span(offset, rows)
            .expect("滚到后当前匹配应可见");
        let text: String = app.session().frame.rows[visible.row]
            .cells
            .iter()
            .map(|cell| cell.text.as_str())
            .collect();
        assert!(text.contains("350"), "高亮行内容应是 350 行");
        assert!(search(app).label().contains("1/1"), "label 应显示 1/1");
    });
}

#[gpui::test]
fn search_option_keys_toggle_case_and_regex(cx: &mut gpui::TestAppContext) {
    let app = cx.new(SiskinApp::new);
    app.update(cx, |app, _| {
        app.open_search();
        let send = |app: &mut SiskinApp, text: &str| {
            app.handle_key(&Keystroke::parse(text).expect("测试里的按键写法必须合法"))
        };
        send(app, "alt-c");
        assert!(search(app).case_sensitive, "Alt+C 应打开大小写敏感");
        assert!(
            search(app).label().contains("Aa"),
            "label 应露出 Aa 开关状态"
        );
        assert_eq!(search(app).query.as_str(), "");
        send(app, "alt-r");
        assert!(search(app).regex, "Alt+R 应打开正则模式");
        // 正则模式下输非法正则：label 显示错误，不崩溃，查询词保留。
        app.search_type('[');
        assert_eq!(search(app).query.as_str(), "[");
        assert!(search(app).error.is_some(), "非法正则应记下错误");
        assert!(
            search(app).label().contains("正则错误"),
            "label 必须露出正则错误"
        );
        send(app, "alt-c");
        assert!(!search(app).case_sensitive, "再按 Alt+C 应关掉大小写敏感");
    });
}
