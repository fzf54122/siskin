// @Time    : 2026/11/5 16:05
// @Author  : fzf
// @FileName: mod.rs
// @Software: Siskin

//! `alacritty_terminal` 的唯一出口。其它模块只看见 `Frame` / `CellView`
//! 这些自己的类型，升级或替换后端时只改这个目录。
//!
//! - `event.rs`：后端事件与网格尺寸
//! - `view.rs`：把 `Term` 读成纯数据快照

mod event;
mod view;

pub(crate) use alacritty_terminal::vte::ansi::CursorShape;
pub(crate) use event::{ScreenSize, VtEvent, VtListener};
#[cfg(test)]
pub(crate) use view::CellView;
#[cfg(test)]
pub(crate) use view::OfflineScreen;
pub(crate) use view::{CellStyle, CursorView, Frame, RowView, frame};

use alacritty_terminal::term::TermMode;

use crate::domain::key::KeyModes;

/// 把后端模式翻译成按键编码需要的模式集合。
pub(crate) fn key_modes(mode: TermMode) -> KeyModes {
    KeyModes {
        app_cursor: mode.contains(TermMode::APP_CURSOR),
        app_keypad: mode.contains(TermMode::APP_KEYPAD),
        kitty_keyboard: mode.contains(TermMode::KITTY_KEYBOARD_PROTOCOL),
        alt_screen: mode.contains(TermMode::ALT_SCREEN),
        bracketed_paste: mode.contains(TermMode::BRACKETED_PASTE),
        mouse_click: mode.contains(TermMode::MOUSE_REPORT_CLICK),
        mouse_drag: mode.contains(TermMode::MOUSE_DRAG),
        mouse_motion: mode.contains(TermMode::MOUSE_MOTION),
        sgr_mouse: mode.contains(TermMode::SGR_MOUSE),
        utf8_mouse: mode.contains(TermMode::UTF8_MOUSE),
    }
}

#[cfg(test)]
mod tests {
    use super::view::OfflineScreen;
    use super::*;
    use crate::io::ansi::AnsiColors;
    use crate::theme::PAPER;

    fn screen() -> OfflineScreen {
        let (sender, _receiver) = async_channel::unbounded();
        OfflineScreen::new(ScreenSize::new(20, 4, 0), sender, 0)
    }

    fn palette() -> AnsiColors {
        AnsiColors::from_palette(&PAPER)
    }

    #[test]
    fn plain_text_lands_in_the_grid() {
        let mut screen = screen();
        screen.feed(b"hello");
        let frame = screen.frame(&palette());
        let text: String = frame.rows[0]
            .cells
            .iter()
            .map(|c| c.text.as_str())
            .collect();
        assert!(text.starts_with("hello"));
        assert_eq!(frame.cursor.column, 5);
    }

    #[test]
    fn sgr_colors_reach_the_frame() {
        let mut screen = screen();
        screen.feed(b"\x1b[31mred\x1b[0m");
        let frame = screen.frame(&palette());
        let table = palette();
        assert_eq!(frame.rows[0].cells[0].style.fg, table.normal[1]);
        assert_eq!(frame.rows[0].cells[0].text, "r");
        assert_ne!(frame.rows[0].cells[3].style.fg, table.normal[1]);
    }

    #[test]
    fn cursor_style_and_position_follow_escape_sequences() {
        let mut screen = screen();
        screen.feed(b"\x1b[2;3H\x1b[4 q");
        let frame = screen.frame(&palette());
        assert_eq!(frame.cursor.row, 1);
        assert_eq!(frame.cursor.column, 2);
        assert_eq!(frame.cursor.shape, CursorShape::Underline);
    }

    #[test]
    fn wide_characters_keep_their_spacer() {
        let mut screen = screen();
        screen.feed("中文".as_bytes());
        let frame = screen.frame(&palette());
        assert_eq!(frame.rows[0].cells[0].text, "中");
        assert!(frame.rows[0].cells[1].spacer);
        assert_eq!(frame.rows[0].cells[2].text, "文");
    }

    #[test]
    fn modes_are_exposed_for_key_encoding() {
        let mut screen = screen();
        screen.feed(b"\x1b[?1h\x1b[?2004h");
        let modes = key_modes(screen.mode());
        assert!(modes.app_cursor);
        assert!(modes.bracketed_paste);
    }

    #[test]
    fn osc8_hyperlinks_reach_the_frame() {
        let mut screen = screen();
        // OSC 8 的 `id` 段可省略；这里刻意带上，确认两种形态都能解析。
        screen.feed(b"\x1b]8;;https://example.com\x1b\\link\x1b]8;;\x1b\\");
        let frame = screen.frame(&palette());
        let cells: Vec<_> = frame.rows[0].cells.iter().collect();
        let linked: Vec<_> = cells.iter().filter(|cell| cell.link.is_some()).collect();
        assert_eq!(linked.len(), 4, "链接段应有 4 个 cell 带地址");
        assert_eq!(linked[0].link.as_deref(), Some("https://example.com"));
        // 链接闭合后，后面的 cell 不能再带地址，否则点击会误伤。
        let plain = cells.iter().skip(4).find(|cell| !cell.text.is_empty());
        if let Some(plain) = plain {
            assert!(plain.link.is_none());
        }
    }

    #[test]
    fn screen_size_clamps_to_a_usable_grid() {
        use alacritty_terminal::grid::Dimensions;
        let size = ScreenSize::new(0, 0, 100);
        assert_eq!(size.columns(), 2);
        assert_eq!(size.screen_lines(), 1);
        assert_eq!(size.total_lines(), 101);
    }

    fn row_text(frame: &Frame, row: usize) -> String {
        frame.rows[row]
            .cells
            .iter()
            .map(|c| c.text.as_str())
            .collect()
    }

    #[test]
    fn cjk_text_occupies_two_columns_and_the_next_char_follows() {
        let mut screen = screen();
        screen.feed("中文a".as_bytes());
        let frame = screen.frame(&palette());
        assert_eq!(frame.rows[0].cells[0].text, "中");
        assert!(frame.rows[0].cells[1].spacer, "「中」的后半格必须是 spacer");
        assert_eq!(frame.rows[0].cells[2].text, "文");
        assert!(frame.rows[0].cells[3].spacer, "「文」的后半格必须是 spacer");
        assert_eq!(frame.rows[0].cells[4].text, "a", "后续字符必须落在第 5 列");
        assert_eq!(frame.cursor.column, 5, "光标应走过 2+2+1 列");
        assert_eq!(row_text(&frame, 0).trim_end(), "中文a");
    }

    #[test]
    fn emoji_occupies_two_columns_and_the_next_char_follows() {
        let mut screen = screen();
        screen.feed("😀a".as_bytes());
        let frame = screen.frame(&palette());
        assert_eq!(frame.rows[0].cells[0].text, "😀");
        assert!(frame.rows[0].cells[1].spacer, "Emoji 的后半格必须是 spacer");
        assert_eq!(frame.rows[0].cells[2].text, "a", "后续字符必须落在第 3 列");
        assert_eq!(frame.cursor.column, 3, "光标应走过 2+1 列");
    }

    #[test]
    fn combining_marks_stay_in_the_same_cell_as_their_base() {
        let mut screen = screen();
        screen.feed("e\u{301}a".as_bytes());
        let frame = screen.frame(&palette());
        assert_eq!(
            frame.rows[0].cells[0].text, "e\u{301}",
            "组合字符不能另占一格"
        );
        assert!(!frame.rows[0].cells[0].spacer);
        assert_eq!(frame.rows[0].cells[1].text, "a", "后续字符必须落在第 2 列");
        assert_eq!(frame.cursor.column, 2, "组合字符不推进光标");
    }

    #[test]
    fn nerd_font_icons_occupy_one_column_like_other_symbols() {
        let mut screen = screen();
        screen.feed("\u{e0b0}a".as_bytes());
        let frame = screen.frame(&palette());
        assert_eq!(frame.rows[0].cells[0].text, "\u{e0b0}");
        assert!(!frame.rows[0].cells[0].spacer, "私用区图标不应被当成宽字符");
        assert_eq!(frame.rows[0].cells[1].text, "a", "后续字符必须落在第 2 列");
        assert_eq!(frame.cursor.column, 2, "图标只占一列");
    }

    #[test]
    fn mixed_width_text_keeps_cursor_and_columns_aligned() {
        let mut screen = screen();
        screen.feed("a中😀b".as_bytes());
        let frame = screen.frame(&palette());
        // 列：a=0、中=1、spacer=2、😀=3、spacer=4、b=5。
        assert_eq!(frame.rows[0].cells[1].text, "中");
        assert_eq!(frame.rows[0].cells[3].text, "😀");
        assert_eq!(frame.rows[0].cells[5].text, "b");
        assert_eq!(frame.cursor.column, 6);
        assert_eq!(row_text(&frame, 0).trim_end(), "a中😀b");
    }
}
