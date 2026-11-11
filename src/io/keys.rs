// @Time    : 2026/11/5 15:45
// @Author  : fzf
// @FileName: keys.rs
// @Software: Siskin

//! 按键 → 终端字节。普通字符写自己，`Ctrl+字母` 写控制码，`Alt+键` 加
//! `ESC` 前缀；方向键与功能键按 `KeyModes` 在 CSI 与 SS3 之间切换。
//!
//! 序列取自 xterm 的 ctlseqs，与 Alacritty / Zed 的映射保持一致，这样
//! `vim`、`htop`、`tmux` 对同一个按键得到相同字节。

use crate::domain::key::{KeyInput, KeyModes};

/// 终端的「修饰键编码」：1 位 Shift，2 位 Alt，4 位 Control，再加一。
fn modifier_code(input: &KeyInput) -> u32 {
    let mut code = 0;
    if input.shift {
        code |= 1;
    }
    if input.alt {
        code |= 2;
    }
    if input.control || input.platform {
        code |= 4;
    }
    code + 1
}

fn any_modifier(input: &KeyInput) -> bool {
    input.control || input.platform || input.alt || input.shift
}

pub(crate) fn encode(input: &KeyInput, modes: &KeyModes) -> Option<Vec<u8>> {
    // Ctrl/Alt 组合必须先于固定序列判断，否则 `Ctrl+Up` 会被当成普通方向键。
    let plain = !(input.control || input.alt || input.platform);
    if plain && let Some(bytes) = plain_special(input, modes) {
        return Some(bytes);
    }
    if any_modifier(input)
        && let Some(bytes) = modified_special(input)
    {
        return Some(bytes);
    }
    if let Some(bytes) = control_character(input) {
        return Some(bytes);
    }
    if input.alt
        && let Some(text) = input.key_char.as_deref().filter(|text| !text.is_empty())
    {
        let mut bytes = Vec::with_capacity(text.len() + 1);
        bytes.push(0x1b);
        bytes.extend_from_slice(text.as_bytes());
        return Some(bytes);
    }
    if any_modifier(input) {
        return None;
    }
    let text = input.key_char.as_deref().unwrap_or(&input.key);
    if text.is_empty() || text.chars().any(char::is_control) {
        return None;
    }
    Some(text.as_bytes().to_vec())
}

/// 不带修饰键的固定序列。
fn plain_special(input: &KeyInput, modes: &KeyModes) -> Option<Vec<u8>> {
    let cursor_prefix = if modes.app_cursor { "O" } else { "[" };
    let sequence: &str = match (input.key.as_str(), input.shift) {
        ("tab", false) => "\t",
        ("tab", true) => "\x1b[Z",
        ("escape", false) => "\x1b",
        ("enter", false) => "\r",
        ("enter", true) => "\n",
        ("backspace", false) => "\x7f",
        ("backspace", true) => "\x7f",
        ("up", false) => return Some(format!("\x1b{cursor_prefix}A").into_bytes()),
        ("down", false) => return Some(format!("\x1b{cursor_prefix}B").into_bytes()),
        ("right", false) => return Some(format!("\x1b{cursor_prefix}C").into_bytes()),
        ("left", false) => return Some(format!("\x1b{cursor_prefix}D").into_bytes()),
        ("home", false) => return Some(format!("\x1b{cursor_prefix}H").into_bytes()),
        ("end", false) => return Some(format!("\x1b{cursor_prefix}F").into_bytes()),
        ("insert", false) => "\x1b[2~",
        ("delete", false) => "\x1b[3~",
        ("pageup", false) => "\x1b[5~",
        ("pagedown", false) => "\x1b[6~",
        ("f1", false) => "\x1bOP",
        ("f2", false) => "\x1bOQ",
        ("f3", false) => "\x1bOR",
        ("f4", false) => "\x1bOS",
        ("f5", false) => "\x1b[15~",
        ("f6", false) => "\x1b[17~",
        ("f7", false) => "\x1b[18~",
        ("f8", false) => "\x1b[19~",
        ("f9", false) => "\x1b[20~",
        ("f10", false) => "\x1b[21~",
        ("f11", false) => "\x1b[23~",
        ("f12", false) => "\x1b[24~",
        _ => return None,
    };
    Some(sequence.as_bytes().to_vec())
}

/// 方向键与功能键带上修饰键时的 CSI 形式。
fn modified_special(input: &KeyInput) -> Option<Vec<u8>> {
    let code = modifier_code(input);
    let sequence = match input.key.as_str() {
        "up" => format!("\x1b[1;{code}A"),
        "down" => format!("\x1b[1;{code}B"),
        "right" => format!("\x1b[1;{code}C"),
        "left" => format!("\x1b[1;{code}D"),
        "home" => format!("\x1b[1;{code}H"),
        "end" => format!("\x1b[1;{code}F"),
        "insert" => format!("\x1b[2;{code}~"),
        "delete" => format!("\x1b[3;{code}~"),
        "pageup" => format!("\x1b[5;{code}~"),
        "pagedown" => format!("\x1b[6;{code}~"),
        "enter" => format!("\x1b[13;{code}u"),
        "backspace" => format!("\x1b[127;{code}u"),
        "tab" => format!("\x1b[9;{code}u"),
        "f1" => format!("\x1b[1;{code}P"),
        "f2" => format!("\x1b[1;{code}Q"),
        "f3" => format!("\x1b[1;{code}R"),
        "f4" => format!("\x1b[1;{code}S"),
        "f5" => format!("\x1b[15;{code}~"),
        "f6" => format!("\x1b[17;{code}~"),
        "f7" => format!("\x1b[18;{code}~"),
        "f8" => format!("\x1b[19;{code}~"),
        "f9" => format!("\x1b[20;{code}~"),
        "f10" => format!("\x1b[21;{code}~"),
        "f11" => format!("\x1b[23;{code}~"),
        "f12" => format!("\x1b[24;{code}~"),
        _ => return None,
    };
    Some(sequence.into_bytes())
}

/// `Ctrl+字母` 与 `Ctrl+标点` 对应的 C0 控制码。
fn control_character(input: &KeyInput) -> Option<Vec<u8>> {
    if !(input.control || input.platform) || input.alt {
        return None;
    }
    let key = input.key_char.as_deref().unwrap_or(&input.key);
    let mut chars = key.chars();
    let ch = chars.next()?;
    if chars.next().is_some() {
        return None;
    }
    let control = match ch.to_ascii_lowercase() {
        'a'..='z' => (ch.to_ascii_lowercase() as u8) - b'a' + 1,
        '@' | ' ' => 0,
        '[' => 0x1b,
        '\\' => 0x1c,
        ']' => 0x1d,
        '^' => 0x1e,
        '_' => 0x1f,
        '?' => 0x7f,
        _ => return None,
    };
    Some(vec![control])
}

/// 粘贴文本。多行粘贴必须先把换行统一成 `\r`，否则 shell 会在读到
/// 第一行时就开始执行；开启 bracketed paste 时再包上边界序列。
pub(crate) fn wrap_paste(text: &str, bracketed: bool) -> Vec<u8> {
    let normalized = text.replace("\r\n", "\r").replace('\n', "\r");
    if !bracketed {
        return normalized.into_bytes();
    }
    let mut bytes = Vec::with_capacity(normalized.len() + 12);
    bytes.extend_from_slice(b"\x1b[200~");
    bytes.extend_from_slice(normalized.as_bytes());
    bytes.extend_from_slice(b"\x1b[201~");
    bytes
}

/// 鼠标上报序列。只在应用开启鼠标模式时使用，否则鼠标留给本地选区。
#[allow(clippy::too_many_arguments)]
pub(crate) fn mouse_report(
    modes: &KeyModes,
    button: u8,
    column: usize,
    row: usize,
    pressed: bool,
    shift: bool,
    alt: bool,
    control: bool,
) -> Option<Vec<u8>> {
    if !modes.mouse_reporting() {
        return None;
    }
    let mut code = button;
    if shift {
        code |= 4;
    }
    if alt {
        code |= 8;
    }
    if control {
        code |= 16;
    }
    if !pressed && !modes.mouse_drag && !modes.mouse_motion && !modes.sgr_mouse {
        // 普通点击模式下只上报按下，松开交给应用自己判断。
        // X10 编码没有松开位，直接上报会看起来像又按了一次。
        // SGR 模式除外：它用 M/m 区分按下与松开，松开必须上报。
        return None;
    } else if !pressed {
        code |= 3;
    }
    let column = column + 1;
    let row = row + 1;
    if modes.sgr_mouse {
        let final_byte = if pressed { 'M' } else { 'm' };
        return Some(format!("\x1b[<{code};{column};{row}{final_byte}").into_bytes());
    }
    // 老式 X10 编码：坐标要加 32，且只能表示 223 以内。
    let column = (column.min(223) + 32) as u8;
    let row = (row.min(223) + 32) as u8;
    Some(vec![0x1b, b'[', b'M', 32 + code, column, row])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn modes() -> KeyModes {
        KeyModes::default()
    }

    fn encode_key(input: KeyInput) -> Option<Vec<u8>> {
        encode(&input, &modes())
    }

    #[test]
    fn plain_characters_pass_through() {
        assert_eq!(
            encode_key(KeyInput::new("a").char("a")).as_deref(),
            Some(b"a".as_slice())
        );
        assert_eq!(
            encode_key(KeyInput::new("中").char("中")).as_deref(),
            Some("中".as_bytes())
        );
    }

    #[test]
    fn control_letters_become_c0_codes() {
        assert_eq!(
            encode_key(KeyInput::new("c").char("c").ctrl()).as_deref(),
            Some([0x03].as_slice())
        );
        assert_eq!(
            encode_key(KeyInput::new("z").char("z").ctrl()).as_deref(),
            Some([0x1a].as_slice())
        );
        assert_eq!(
            encode_key(KeyInput::new("space").char(" ").ctrl()).as_deref(),
            Some([0x00].as_slice())
        );
    }

    #[test]
    fn arrows_follow_the_cursor_mode() {
        assert_eq!(
            encode_key(KeyInput::new("up")).as_deref(),
            Some(b"\x1b[A".as_slice())
        );
        let app = KeyModes {
            app_cursor: true,
            ..KeyModes::default()
        };
        assert_eq!(
            encode(&KeyInput::new("up"), &app).as_deref(),
            Some(b"\x1bOA".as_slice())
        );
    }

    #[test]
    fn modified_arrows_carry_a_modifier_code() {
        assert_eq!(
            encode_key(KeyInput::new("up").ctrl()).as_deref(),
            Some(b"\x1b[1;5A".as_slice())
        );
        assert_eq!(
            encode_key(KeyInput::new("right").shift().alt()).as_deref(),
            Some(b"\x1b[1;4C".as_slice())
        );
    }

    #[test]
    fn alt_prefixes_the_escape_byte() {
        assert_eq!(
            encode_key(KeyInput::new("b").char("b").alt()).as_deref(),
            Some(b"\x1bb".as_slice())
        );
    }

    #[test]
    fn unbound_modifier_combinations_are_left_to_the_framework() {
        // Ctrl+X 是合法的 C0 控制码，继续下发。
        assert_eq!(
            encode_key(KeyInput::new("x").char("x").ctrl()).as_deref(),
            Some([0x18].as_slice())
        );
        // 未知键名带修饰键时找不到映射，交还框架。
        assert!(encode_key(KeyInput::new("f35").ctrl()).is_none());
    }

    #[test]
    fn paste_normalizes_newlines_and_wraps_when_requested() {
        assert_eq!(wrap_paste("a\r\nb\nc", false), b"a\rb\rc".to_vec());
        assert_eq!(wrap_paste("a\nb", true), b"\x1b[200~a\rb\x1b[201~".to_vec());
    }

    #[test]
    fn mouse_reports_only_when_the_application_asks() {
        let mut modes = KeyModes {
            sgr_mouse: true,
            mouse_click: true,
            ..KeyModes::default()
        };
        assert_eq!(
            mouse_report(&modes, 0, 4, 2, true, false, false, false).as_deref(),
            Some(b"\x1b[<0;5;3M".as_slice())
        );
        modes.mouse_click = false;
        assert!(mouse_report(&modes, 0, 0, 0, true, false, false, false).is_none());
    }
}
