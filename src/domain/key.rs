// @Time    : 2026/11/5 15:40
// @Author  : fzf
// @FileName: key.rs
// @Software: Siskin

//! 与 GUI 框架无关的按键描述。`ui` 层把 GPUI 的 `Keystroke` 归一成它，
//! `io::keys` 只依赖这个结构，因此编码逻辑可以在无显示器环境下测试。

/// 终端需要知道的应用模式。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct KeyModes {
    pub app_cursor: bool,
    pub app_keypad: bool,
    pub kitty_keyboard: bool,
    pub alt_screen: bool,
    pub bracketed_paste: bool,
    pub mouse_click: bool,
    pub mouse_drag: bool,
    pub mouse_motion: bool,
    pub sgr_mouse: bool,
    pub utf8_mouse: bool,
}

impl KeyModes {
    pub(crate) fn mouse_reporting(self) -> bool {
        self.mouse_click || self.mouse_drag || self.mouse_motion
    }
}

/// 一次按键。`key` 是归一后的键名（`a`、`enter`、`up`…），`key_char`
/// 是平台给出的实际字符，输入法提交的文本不走这里。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct KeyInput {
    pub key: String,
    pub key_char: Option<String>,
    pub control: bool,
    pub alt: bool,
    pub shift: bool,
    pub platform: bool,
}

impl KeyInput {
    #[cfg(test)]
    pub(crate) fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            key_char: None,
            control: false,
            alt: false,
            shift: false,
            platform: false,
        }
    }

    #[cfg(test)]
    pub(crate) fn ctrl(mut self) -> Self {
        self.control = true;
        self
    }

    #[cfg(test)]
    pub(crate) fn alt(mut self) -> Self {
        self.alt = true;
        self
    }

    #[cfg(test)]
    pub(crate) fn shift(mut self) -> Self {
        self.shift = true;
        self
    }

    #[cfg(test)]
    pub(crate) fn char(mut self, text: impl Into<String>) -> Self {
        self.key_char = Some(text.into());
        self
    }
}
