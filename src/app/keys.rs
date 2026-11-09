// @Time    : 2026/11/5 17:00
// @Author  : fzf
// @FileName: keys.rs
// @Software: Siskin

//! 按键责任链。顺序是：颜色编辑 → 命令面板 → 全局快捷键 → 设置页 → 终端。
//! 每一环返回 `Some` 表示已消费，`None` 交给下一环。
//!
//! 终端这一环先查自己的快捷键（`Ctrl+Shift+*`），未命中才把原始按键交给
//! `io::keys::encode` 转成 PTY 字节；顺序反了 `Ctrl+R`、`Ctrl+P` 会被吞掉。

use gpui::Keystroke;

use crate::domain::Action;
use crate::domain::key::{KeyInput, KeyModes};
use crate::io::keys;
use crate::settings::Page;

use super::SiskinApp;

/// 一次按键的归一化结果。
#[derive(Clone, Debug)]
pub(crate) struct KeyPress<'a> {
    pub key: &'a str,
    pub key_char: Option<&'a str>,
    pub control: bool,
    pub shift: bool,
    pub alt: bool,
    pub platform: bool,
}

impl<'a> KeyPress<'a> {
    pub(crate) fn new(keystroke: &'a Keystroke) -> Self {
        Self {
            key: keystroke.key.as_str(),
            key_char: keystroke.key_char.as_deref(),
            control: keystroke.modifiers.control,
            shift: keystroke.modifiers.shift,
            alt: keystroke.modifiers.alt,
            platform: keystroke.modifiers.platform,
        }
    }

    fn to_input(&self) -> KeyInput {
        KeyInput {
            key: self.key.to_string(),
            key_char: self.key_char.map(str::to_string),
            control: self.control,
            alt: self.alt,
            shift: self.shift,
            platform: self.platform,
        }
    }

    /// 带 Ctrl / Alt / 平台的组合。方向键和功能键不算「修饰」。
    pub(super) fn modified(&self) -> bool {
        self.control || self.alt || self.platform
    }
}

pub(crate) enum Handled {
    /// 已消费，界面需要重绘。
    Redraw,
    /// 已消费并退出应用。
    Quit,
}

type KeyStage = fn(&mut SiskinApp, KeyPress<'_>) -> Option<Handled>;

const STAGES: [KeyStage; 7] = [
    close_stage,
    color_edit_stage,
    search_stage,
    palette_stage,
    shortcut_stage,
    settings_stage,
    terminal_stage,
];

impl SiskinApp {
    /// 依次尝试每一环。返回 `true` 表示按键已被消费。
    pub(crate) fn handle_key(&mut self, keystroke: &Keystroke) -> Option<Handled> {
        let press = KeyPress::new(keystroke);
        STAGES.iter().find_map(|stage| stage(self, press.clone()))
    }
}

/// 取色输入框必须排在最前，否则 `Ctrl+A` 会被当成终端动作。
fn color_edit_stage(app: &mut SiskinApp, press: KeyPress<'_>) -> Option<Handled> {
    app.editor()?;
    app.edit_key(&press);
    Some(Handled::Redraw)
}

fn close_stage(app: &mut SiskinApp, press: KeyPress<'_>) -> Option<Handled> {
    app.close_request?;
    if press.key == "escape" {
        app.cancel_close();
    }
    Some(Handled::Redraw)
}

/// 搜索条打开时接管字符输入，`Esc` 关闭，`Enter` 跳到下一个匹配。
fn search_stage(app: &mut SiskinApp, press: KeyPress<'_>) -> Option<Handled> {
    app.search.as_ref()?;
    match press.key {
        "escape" => app.close_search(),
        "enter" => app.step_match(!press.shift),
        "up" => app.step_match(false),
        "down" => app.step_match(true),
        "backspace" => app.search_backspace(),
        _ => {
            if press.modified() {
                return None;
            }
            // 有的平台只给键名不给字符；单字符键名同样可以插入。
            let ch = press
                .key_char
                .and_then(single_char)
                .or_else(|| single_char(press.key))?;
            app.search_type(ch);
        }
    }
    Some(Handled::Redraw)
}

fn palette_stage(app: &mut SiskinApp, press: KeyPress<'_>) -> Option<Handled> {
    app.palette.as_ref()?;
    match press.key {
        "escape" => app.close_palette(),
        "enter" => app.palette_confirm(),
        "up" => app.palette_move(false),
        "down" => app.palette_move(true),
        "backspace" => app.palette_backspace(),
        _ => {
            // 面板里的 Ctrl/Alt 组合留给全局快捷键，例如 Ctrl+K 关面板后重开。
            if press.modified() {
                return None;
            }
            let ch = press.key_char.and_then(single_char)?;
            app.palette_type(ch);
        }
    }
    Some(Handled::Redraw)
}

/// 终端动作统一走 `Ctrl+Shift+*`，避开 shell 与 readline 常用键。
fn shortcut_stage(app: &mut SiskinApp, press: KeyPress<'_>) -> Option<Handled> {
    let action = match (press.control, press.shift, press.key) {
        (true, false, "k") => Action::OpenPalette,
        (true, false, ",") => Action::OpenSettings,
        (true, false, "1") => Action::OpenTerminal,
        (true, true, "i") => Action::ToggleInspector,
        (true, true, "t") => Action::CycleThemeMode,
        (true, true, "f") => Action::OpenSearch,
        (true, true, "n") => Action::NewSession,
        (true, true, "w") => Action::CloseSession,
        (true, false, "=") | (true, false, "+") => Action::FontLarger,
        (true, false, "-") => Action::FontSmaller,
        (true, false, "tab") => Action::NextSession,
        (true, true, "tab") => Action::PreviousSession,
        _ => return None,
    };
    Some(match app.dispatch(action) {
        crate::domain::ActionEffect::Quit => Handled::Quit,
        crate::domain::ActionEffect::Redraw => Handled::Redraw,
    })
}

/// 设置页拥有自己的方向键；终端不能在这里吞掉用户的浏览操作。
fn settings_stage(app: &mut SiskinApp, press: KeyPress<'_>) -> Option<Handled> {
    if app.page != Page::Settings {
        return None;
    }
    match press.key {
        "escape" => {
            Action::OpenTerminal.run(app);
        }
        _ => return None,
    }
    Some(Handled::Redraw)
}

/// `Shift+PageUp/PageDown/Home/End` 是终端自己的滚动快捷键，优先于程序输入。
fn scroll_shortcut(app: &mut SiskinApp, press: KeyPress<'_>) -> Option<Handled> {
    if !press.shift || press.control {
        return None;
    }
    let action = match press.key {
        "pageup" => Action::ScrollPageUp,
        "pagedown" => Action::ScrollPageDown,
        "home" => Action::ScrollToTop,
        "end" => Action::ScrollToBottom,
        _ => return None,
    };
    app.dispatch(action);
    Some(Handled::Redraw)
}

fn terminal_stage(app: &mut SiskinApp, press: KeyPress<'_>) -> Option<Handled> {
    if app.page != Page::Terminal {
        return None;
    }
    if let Some(handled) = scroll_shortcut(app, press.clone()) {
        return Some(handled);
    }
    if !app.session().is_running() {
        // 进程已退出：Enter 关闭会话，最后一个交给窗口退出，
        // 其它按键不再下发，避免写进一个没有读者的 PTY。
        if press.key == "enter" {
            let index = app.active_index();
            app.close_session(index);
            return Some(Handled::Redraw);
        }
        return None;
    }
    let modes: KeyModes = app
        .session()
        .pty()
        .map(|pty| crate::io::vt::key_modes(pty.mode()))
        .unwrap_or_default();
    let bytes = keys::encode(&press.to_input(), &modes)?;
    app.write_bytes(&bytes);
    app.scroll_to_bottom();
    Some(Handled::Redraw)
}

fn single_char(text: &str) -> Option<char> {
    let mut chars = text.chars();
    let first = chars.next()?;
    chars.next().is_none().then_some(first)
}
