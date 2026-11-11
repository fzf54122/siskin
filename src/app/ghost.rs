// @Time    : 2026/11/11
// @Author  : fzf
// @FileName: ghost.rs
// @Software: Siskin

//! 行内自动补全（ghost text）的应用层：从当前帧读输入行、
//! 按会话状态算联想后缀、接管 Tab / Right / Esc。
//!
//! 关键约束：ghost 只画出来，不写进 PTY、不动光标、不改终端内容；
//! 接受前绝不向 shell 发任何字节。接受时把后缀当成普通键入字节写进
//! PTY（与用户自己敲出这些字等价），shell 自己的行编辑负责回显。

use crate::domain::ghost::{GhostState, first_word};
use crate::io::vt::Frame;

use super::keys::{Handled, KeyPress};
use super::{Session, SiskinApp};

/// 从帧里读当前输入行：提示符锚点所在行、光标列之前、去掉提示符。
/// 锚点行与光标行对不上（换行输入、命令已跑起来）、光标不在行尾、
/// 滚进历史、IME 预编辑中时都返回 `None`，不猜、不补。
fn current_input(
    frame: &Frame,
    history_size: usize,
    state: &GhostState,
    marked: bool,
) -> Option<String> {
    let anchor = state.prompt()?;
    if marked || frame.display_offset > 0 || !frame.cursor.visible {
        return None;
    }
    let cursor_line = history_size as i32 + frame.cursor.row as i32;
    if cursor_line != anchor.line {
        return None;
    }
    let row = frame.rows.get(frame.cursor.row)?;
    let (prompt, cursor) = (anchor.column, frame.cursor.column);
    if cursor < prompt {
        return None;
    }
    let after: String = row
        .cells
        .get(cursor..)
        .map(|cells| cells.iter().map(|cell| cell.text.as_str()).collect())
        .unwrap_or_default();
    if !after.trim().is_empty() {
        return None;
    }
    let input: String = row
        .cells
        .get(prompt..cursor)
        .map(|cells| cells.iter().map(|cell| cell.text.as_str()).collect())
        .unwrap_or_default();
    if input.is_empty() {
        return None;
    }
    Some(input)
}

/// 给定帧与状态算出联想后缀。`history_size` 是当前滚动区行数，
/// 没有 PTY 的会话按 0 算（它从来没有输出过，历史就是空的）。
/// 不依赖 GPUI，测试可以直接拼帧调用。
pub(crate) fn ghost_suffix_for(
    frame: &Frame,
    history_size: usize,
    state: &GhostState,
    marked: bool,
) -> Option<String> {
    let input = current_input(frame, history_size, state, marked)?;
    state.suggest(&input)
}

impl Session {
    /// 当前帧里的联想后缀：只画在光标后面的那段。
    pub(crate) fn ghost_suffix(&self) -> Option<String> {
        ghost_suffix_for(
            &self.frame,
            self.history_size().unwrap_or(0),
            &self.ghost,
            self.marked.is_some(),
        )
    }

    /// 当前输入与它的联想后缀。Esc 取消时要按输入记，Tab / Right
    /// 接受时要按输入算后缀，分开取会与帧状态对不上。
    pub(crate) fn ghost_input_and_suffix(&self) -> Option<(String, String)> {
        let history_size = self.history_size().unwrap_or(0);
        let input = current_input(
            &self.frame,
            history_size,
            &self.ghost,
            self.marked.is_some(),
        )?;
        let suffix = self.ghost.suggest(&input)?;
        Some((input, suffix))
    }

    /// Esc 取消当前联想：同一输入不再联想，直到输入变化。
    pub(crate) fn dismiss_ghost(&mut self) {
        let history_size = self.history_size().unwrap_or(0);
        if let Some(input) = current_input(
            &self.frame,
            history_size,
            &self.ghost,
            self.marked.is_some(),
        ) {
            self.ghost.dismiss(&input);
        }
    }
}

impl SiskinApp {
    /// ghost 键位：无修饰的 Tab 接受整条、Right 接受一个词、
    /// Esc 取消当前联想。带修饰的键一律放行，不与 shell 抢。
    /// 有联想时才消费按键；没有时返回 `None`，原样进 PTY。
    /// 调用方已确认命令面板 / 搜索条都没打开。
    pub(crate) fn ghost_key(&mut self, press: &KeyPress<'_>) -> Option<Handled> {
        if press.modified() {
            return None;
        }
        match press.key {
            "tab" | "right" => {
                let (_, suffix) = self.session().ghost_input_and_suffix()?;
                let text = if press.key == "tab" {
                    suffix
                } else {
                    let word = first_word(&suffix);
                    if word.is_empty() {
                        suffix
                    } else {
                        word.to_string()
                    }
                };
                self.write_bytes(text.as_bytes());
                self.scroll_to_bottom();
                Some(Handled::Redraw)
            }
            "escape" => {
                // 只有真有联想时才消费 Esc：否则 Esc 要原样进 PTY，
                // 它是 Alt 组合与程序转义的前缀，不能吞掉。
                self.session().ghost_suffix()?;
                self.session_mut().dismiss_ghost();
                Some(Handled::Redraw)
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::vt::{CellStyle, CellView, CursorView, RowView};
    use alacritty_terminal::vte::ansi::CursorShape;

    fn cell(text: &str) -> CellView {
        CellView {
            text: text.to_string(),
            style: CellStyle::default(),
            spacer: false,
            selected: false,
            link: None,
        }
    }

    fn spacer() -> CellView {
        CellView {
            text: String::new(),
            style: CellStyle::default(),
            spacer: true,
            selected: false,
            link: None,
        }
    }

    /// 帧：首行是 `$ git st`，光标停在 `st` 后面；其余行是空行。
    fn frame(columns: usize) -> Frame {
        let mut cells: Vec<CellView> = "$ git st".chars().map(|ch| cell(&ch.to_string())).collect();
        cells.resize_with(columns, || cell(" "));
        Frame {
            rows: vec![RowView { cells }],
            cursor: CursorView {
                row: 0,
                column: 8,
                shape: CursorShape::Block,
                visible: true,
            },
            display_offset: 0,
        }
    }

    fn state() -> GhostState {
        let mut state = GhostState::new();
        state.record("git status");
        state.record("git stash");
        state.on_input_start(0, 2);
        state
    }

    #[test]
    fn ghost_suffix_comes_from_the_prompt_line() {
        let frame = frame(20);
        assert_eq!(
            ghost_suffix_for(&frame, 0, &state(), false),
            Some("ash".to_string()),
            "最近的 git stash 应优先于 git status"
        );
    }

    #[test]
    fn ghost_is_hidden_when_the_cursor_is_not_at_the_input_line() {
        let frame = frame(20);
        // 光标行不变，锚点行是另一行：命令已换行或已跑起来。
        let mut state = state();
        state.on_input_start(4, 2);
        assert_eq!(ghost_suffix_for(&frame, 0, &state, false), None);
    }

    #[test]
    fn ghost_is_hidden_when_scrolled_into_history() {
        let mut frame = frame(20);
        frame.display_offset = 3;
        frame.cursor.visible = false;
        assert_eq!(ghost_suffix_for(&frame, 0, &state(), false), None);
    }

    #[test]
    fn ghost_is_hidden_when_the_cursor_is_mid_line() {
        let mut frame = frame(20);
        // 光标移到 `git` 后面：行尾还有 ` st`，是行中编辑，不联想。
        frame.cursor.column = 5;
        assert_eq!(ghost_suffix_for(&frame, 0, &state(), false), None);
    }

    #[test]
    fn ghost_is_hidden_during_ime_composition() {
        let frame = frame(20);
        assert_eq!(
            ghost_suffix_for(&frame, 0, &state(), true),
            None,
            "输入法预编辑时不联想"
        );
    }

    #[test]
    fn ghost_is_hidden_for_empty_input() {
        let mut frame = frame(20);
        frame.cursor.column = 2;
        assert_eq!(ghost_suffix_for(&frame, 0, &state(), false), None);
    }

    #[test]
    fn wide_chars_in_the_input_keep_the_columns_aligned() {
        let mut frame = frame(20);
        // 把输入换成 `中文a`：光标在第 2 列之后（中占两格），提示符末尾仍是第 2 列。
        let mut cells = frame.rows[0].cells.clone();
        cells[2] = cell("中");
        cells[3] = spacer();
        cells[4] = cell("文");
        cells[5] = spacer();
        cells[6] = cell("a");
        cells[7] = cell(" ");
        frame.rows[0].cells = cells;
        frame.cursor.column = 7;
        let mut state = GhostState::new();
        state.record("中文ab");
        state.on_input_start(0, 2);
        assert_eq!(
            ghost_suffix_for(&frame, 0, &state, false),
            Some("b".to_string()),
            "宽字符的后半格不能算进输入文本"
        );
    }

    #[test]
    fn cursor_before_the_prompt_end_gives_no_input() {
        let mut frame = frame(20);
        frame.cursor.column = 1;
        assert_eq!(ghost_suffix_for(&frame, 0, &state(), false), None);
    }
}
