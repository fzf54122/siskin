// @Time    : 2026/11/11
// @Author  : fzf
// @FileName: osc133.rs
// @Software: Siskin

//! OSC 133 扫描状态机：只观测、不剥离。原字节照常全部喂给 VT 解析器，
//! 这里只从字节流里认出 `ESC ] 133 ; <params> ST`（ST 是 `ESC \` 或 BEL）。
//!
//! 跨 `read` 分片：一个 OSC 可能分在两次读里，状态与未完成的参数
//! 都保留在扫描器里；非法或超长序列有上限并复位，不卡死、不丢后续字节。
//! 非 133 的 OSC（如 52/8）只跳过，不产生标记。

use crate::domain::shell_marks::Osc133Mark;

/// OSC 133 参数体上限（字节）。正常标记只有 `A` / `D;3` 这类短参数，
/// 超了就是非法输入，直接丢弃整个序列而不是无限攒下去。
const MAX_PARAMS_LEN: usize = 256;

/// `133;` 前缀，逐字节比对。
const PREFIX: &[u8] = b"133;";

/// 一次扫描命中。`anchor` 是快照光标行的字节位置：标记的 `ESC` 在本次
/// 缓冲区里就是它的下标；标记从上次读就开始了则为 0（本次缓冲区开头）。
/// `end` 是终结符之后的位置，`anchor..end` 是标记本身的字节。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Osc133Hit {
    pub anchor: usize,
    pub end: usize,
    pub mark: Osc133Mark,
}

#[derive(Debug)]
enum State {
    Ground,
    Esc,
    /// 正在比对 `133;` 前缀，`matched` 是已对上的字节数。
    Prefix {
        matched: usize,
    },
    /// 前缀已对上，正在攒 `;` 之后的参数。
    Body,
    /// 非 133 的 OSC：只等终结符，不攒参数。
    Skip,
    /// OSC 体里见到了 `ESC`，等下一个字节判定是不是 `ESC \` 终结。
    BodyEsc,
    SkipEsc,
}

#[derive(Debug)]
pub(crate) struct Osc133Scanner {
    state: State,
    params: Vec<u8>,
    /// 候选序列起始 `ESC` 在本次缓冲区里的下标。每次 `scan` 开头清零：
    /// 上次读留下的下标对本次缓冲区没有意义，跨分片的标记锚点就是 0。
    esc_index: Option<usize>,
    /// 体内 `ESC`（进 `BodyEsc` 那个）的下标：它可能是终结符的 `ESC`
    ///（此时锚点仍用 `esc_index`），也可能是新 OSC 的开头（`ESC ]`
    /// 时锚点改成它）。
    body_esc_index: Option<usize>,
}

impl Default for Osc133Scanner {
    fn default() -> Self {
        Self {
            state: State::Ground,
            params: Vec::new(),
            esc_index: None,
            body_esc_index: None,
        }
    }
}

impl Osc133Scanner {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// 扫一段字节，返回其中完成（见到终结符）的全部标记，按出现顺序。
    pub(crate) fn scan(&mut self, bytes: &[u8]) -> Vec<Osc133Hit> {
        self.esc_index = None;
        self.body_esc_index = None;
        let mut hits = Vec::new();
        for (index, &byte) in bytes.iter().enumerate() {
            if let Some(hit) = self.step(byte, index) {
                hits.push(hit);
            }
        }
        hits
    }

    fn step(&mut self, byte: u8, index: usize) -> Option<Osc133Hit> {
        match self.state {
            State::Ground => self.step_ground(byte, index),
            State::Esc => self.step_esc(byte, index),
            State::Prefix { matched } => self.step_prefix(byte, matched, index),
            State::Body => self.step_body(byte, index),
            State::Skip => self.step_skip(byte, index),
            State::BodyEsc => self.step_body_esc(byte, index),
            State::SkipEsc => self.step_skip_esc(byte, index),
        }
    }

    fn step_ground(&mut self, byte: u8, index: usize) -> Option<Osc133Hit> {
        if byte == 0x1b {
            self.esc_index = Some(index);
            self.state = State::Esc;
        }
        None
    }

    fn step_esc(&mut self, byte: u8, index: usize) -> Option<Osc133Hit> {
        match byte {
            b']' => {
                self.state = State::Prefix { matched: 0 };
            }
            0x1b => self.esc_index = Some(index),
            _ => {
                self.esc_index = None;
                self.state = State::Ground;
            }
        }
        None
    }

    fn step_prefix(&mut self, byte: u8, matched: usize, index: usize) -> Option<Osc133Hit> {
        if byte == 0x1b {
            self.esc_index = Some(index);
            self.state = State::Esc;
            return None;
        }
        if byte == 0x07 {
            self.abort();
            return None;
        }
        if matched < PREFIX.len() && byte == PREFIX[matched] {
            let matched = matched + 1;
            if matched == PREFIX.len() {
                self.state = State::Body;
                self.params.clear();
            } else {
                self.state = State::Prefix { matched };
            }
        } else {
            // 前缀对不上：这不是 133，跳到终结符再恢复。
            self.esc_index = None;
            self.state = State::Skip;
        }
        None
    }

    fn step_body(&mut self, byte: u8, index: usize) -> Option<Osc133Hit> {
        match byte {
            0x07 => self.finish(index + 1),
            0x1b => {
                self.body_esc_index = Some(index);
                self.state = State::BodyEsc;
                None
            }
            _ => {
                if self.params.len() >= MAX_PARAMS_LEN {
                    // 超长：丢弃整个序列，等终结符再恢复。
                    self.params.clear();
                    self.esc_index = None;
                    self.state = State::Skip;
                    return None;
                }
                self.params.push(byte);
                None
            }
        }
    }

    fn step_skip(&mut self, byte: u8, index: usize) -> Option<Osc133Hit> {
        match byte {
            0x07 => self.state = State::Ground,
            0x1b => {
                self.esc_index = Some(index);
                self.state = State::SkipEsc;
            }
            _ => {}
        }
        None
    }

    fn step_body_esc(&mut self, byte: u8, index: usize) -> Option<Osc133Hit> {
        match byte {
            b'\\' => self.finish(index + 1),
            b']' => {
                // `ESC ]`：上一个序列作废，这是一个新 OSC 的开头。
                self.params.clear();
                self.esc_index = self.body_esc_index.take();
                self.state = State::Prefix { matched: 0 };
                None
            }
            0x1b => {
                self.body_esc_index = Some(index);
                None
            }
            _ => {
                self.abort();
                None
            }
        }
    }

    fn step_skip_esc(&mut self, byte: u8, index: usize) -> Option<Osc133Hit> {
        match byte {
            b'\\' => self.state = State::Ground,
            b']' => self.state = State::Prefix { matched: 0 },
            0x1b => self.esc_index = Some(index),
            _ => {
                self.esc_index = None;
                self.state = State::Ground;
            }
        }
        None
    }

    /// 序列作废：回地面，锚点清零，不丢后续字节。
    fn abort(&mut self) {
        self.params.clear();
        self.esc_index = None;
        self.body_esc_index = None;
        self.state = State::Ground;
    }

    /// 终结符到达：按参数解析标记，复位状态。
    fn finish(&mut self, end: usize) -> Option<Osc133Hit> {
        let mark = parse_params(&self.params);
        self.params.clear();
        self.body_esc_index = None;
        self.state = State::Ground;
        let anchor = self.esc_index.take().unwrap_or(0);
        mark.map(|mark| Osc133Hit { anchor, end, mark })
    }
}

/// 解析 `133;` 之后的参数。首段必须是单个 `A`/`B`/`C`/`D`，
/// 否则整条作废；`D` 的第二段是退出码，解析失败记 `None`，不猜。
fn parse_params(params: &[u8]) -> Option<Osc133Mark> {
    let first = params.split(|&byte| byte == b';').next()?;
    if first.len() != 1 {
        return None;
    }
    match first[0] {
        b'A' => Some(Osc133Mark::PromptStart),
        b'B' => Some(Osc133Mark::CommandInput),
        b'C' => Some(Osc133Mark::PreExec),
        b'D' => {
            let code = params
                .split(|&byte| byte == b';')
                .nth(1)
                .and_then(|text| std::str::from_utf8(text).ok())
                .and_then(|text| text.parse::<i32>().ok());
            Some(Osc133Mark::CommandEnd { exit_code: code })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scan_all(chunks: &[&[u8]]) -> Vec<Osc133Hit> {
        let mut scanner = Osc133Scanner::new();
        let mut hits = Vec::new();
        for chunk in chunks {
            hits.extend(scanner.scan(chunk));
        }
        hits
    }

    fn marks(hits: &[Osc133Hit]) -> Vec<Osc133Mark> {
        hits.iter().map(|hit| hit.mark).collect()
    }

    #[test]
    fn all_four_kinds_parse_with_bel_terminator() {
        let hits = scan_all(&[b"\x1b]133;A\x07\x1b]133;B\x07\x1b]133;C\x07\x1b]133;D;3\x07"]);
        assert_eq!(
            marks(&hits),
            vec![
                Osc133Mark::PromptStart,
                Osc133Mark::CommandInput,
                Osc133Mark::PreExec,
                Osc133Mark::CommandEnd { exit_code: Some(3) },
            ]
        );
        assert_eq!(hits[0].anchor, 0);
        assert_eq!(hits[0].end, 8);
    }

    #[test]
    fn st_terminator_is_accepted_too() {
        let hits = scan_all(&[b"\x1b]133;A\x1b\\"]);
        assert_eq!(marks(&hits), vec![Osc133Mark::PromptStart]);
        assert_eq!(hits[0].anchor, 0);
        assert_eq!(hits[0].end, 9);
    }

    #[test]
    fn d_without_a_code_and_with_zero_parse() {
        let hits = scan_all(&[b"\x1b]133;D\x07\x1b]133;D;0\x07\x1b]133;D;x\x07"]);
        assert_eq!(
            marks(&hits),
            vec![
                Osc133Mark::CommandEnd { exit_code: None },
                Osc133Mark::CommandEnd { exit_code: Some(0) },
                Osc133Mark::CommandEnd { exit_code: None },
            ]
        );
    }

    #[test]
    fn a_sequence_split_across_reads_still_parses() {
        let hits = scan_all(&[b"\x1b]133;", b"D;3\x07"]);
        assert_eq!(
            marks(&hits),
            vec![Osc133Mark::CommandEnd { exit_code: Some(3) }]
        );
        // 标记从上次读开始，锚点落在本次缓冲区开头。
        assert_eq!(hits[0].anchor, 0);
        assert_eq!(hits[0].end, 4);
    }

    #[test]
    fn a_sequence_split_inside_the_terminator_still_parses() {
        let hits = scan_all(&[b"\x1b]133;A\x1b", b"\\"]);
        assert_eq!(marks(&hits), vec![Osc133Mark::PromptStart]);
        assert_eq!(hits[0].anchor, 0);
        assert_eq!(hits[0].end, 1);
    }

    #[test]
    fn an_invalid_kind_is_dropped_without_breaking_the_next_mark() {
        let hits = scan_all(&[b"\x1b]133;X\x07\x1b]133;A\x07"]);
        assert_eq!(marks(&hits), vec![Osc133Mark::PromptStart]);
    }

    #[test]
    fn an_overlong_sequence_resets_and_later_bytes_survive() {
        let mut input = b"\x1b]133;".to_vec();
        input.extend(std::iter::repeat_n(b'A', MAX_PARAMS_LEN + 10));
        input.extend_from_slice(b"\x07\x1b]133;B\x07");
        let hits = scan_all(&[&input]);
        assert_eq!(marks(&hits), vec![Osc133Mark::CommandInput]);
    }

    #[test]
    fn non_133_osc_sequences_are_ignored() {
        let hits = scan_all(&[b"\x1b]52;c;aGVsbG8=\x07\x1b]8;;https://example.com\x1b\\"]);
        assert!(hits.is_empty(), "OSC 52/8 不能产生 133 标记");
    }

    #[test]
    fn a_bare_escape_and_plain_text_do_not_confuse_the_scanner() {
        let hits = scan_all(&[b"hello \x1b[31mred\x1b[0m world"]);
        assert!(hits.is_empty());
    }

    #[test]
    fn consecutive_marks_keep_their_anchors_in_order() {
        let hits = scan_all(&[b"ab\x1b]133;A\x07cd\x1b]133;D;1\x07"]);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].anchor, 2);
        assert_eq!(hits[1].anchor, 12);
        assert_eq!(hits[1].mark, Osc133Mark::CommandEnd { exit_code: Some(1) });
    }
}
