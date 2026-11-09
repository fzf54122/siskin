// @Time    : 2026/11/5 19:08
// @Author  : fzf
// @FileName: view.rs
// @Software: Siskin

//! 网格快照。把 `alacritty_terminal` 的 `Term` 读成纯数据，渲染层与
//! 测试都只依赖这里的类型，不直接接触后端。

use alacritty_terminal::Term;
use alacritty_terminal::event::EventListener;
use alacritty_terminal::index::{Line, Point};
use alacritty_terminal::term::cell::{Cell, Flags};
use alacritty_terminal::vte::ansi::{Color, CursorShape, Rgb};

use crate::io::ansi::AnsiColors;

/// 一个单元格的最终样式。颜色已经是 RGB，界面不再关心 ANSI 编号。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct CellStyle {
    pub fg: u32,
    pub bg: u32,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikeout: bool,
    pub hidden: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CellView {
    pub text: String,
    pub style: CellStyle,
    /// 宽字符的后半格，绘制时跳过但宽度要算进前一个字。
    pub spacer: bool,
    pub selected: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RowView {
    pub cells: Vec<CellView>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CursorView {
    /// 相对可视区首行的行号。
    pub row: usize,
    pub column: usize,
    pub shape: CursorShape,
}

/// 一帧的完整内容。只在主线程构造，渲染阶段直接读。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Frame {
    pub rows: Vec<RowView>,
    pub cursor: CursorView,
    pub display_offset: usize,
}

impl Frame {
    pub(crate) fn empty(rows: usize) -> Self {
        Self {
            rows: vec![RowView { cells: Vec::new() }; rows],
            cursor: CursorView {
                row: 0,
                column: 0,
                shape: CursorShape::Hidden,
            },
            display_offset: 0,
        }
    }
}

/// 用主题色把一帧读出来。`term` 必须是已锁定的，调用方持有锁。
pub(crate) fn frame<T>(term: &Term<T>, palette: &AnsiColors) -> Frame
where
    T: EventListener,
{
    let content = term.renderable_content();
    let selection = content.selection;
    let cursor = content.cursor;
    let mut rows: Vec<RowView> = Vec::new();
    let mut current_line: Option<Line> = None;

    for indexed in content.display_iter {
        let point = indexed.point;
        if current_line != Some(point.line) {
            current_line = Some(point.line);
            rows.push(RowView { cells: Vec::new() });
        }
        let Some(row) = rows.last_mut() else {
            continue;
        };
        row.cells.push(cell_view(
            indexed.cell,
            point,
            selection,
            palette,
            content.colors,
        ));
    }

    Frame {
        cursor: CursorView {
            row: cursor_row(cursor.point.line, rows.len()),
            column: cursor.point.column.0,
            shape: cursor.shape,
        },
        display_offset: content.display_offset,
        rows,
    }
}

fn cursor_row(line: Line, rows: usize) -> usize {
    // display_iter 的首行是 `-display_offset`，光标行号同样是相对坐标。
    let row = line.0.max(0) as usize;
    row.min(rows.saturating_sub(1))
}

fn cell_view(
    cell: &Cell,
    point: Point,
    selection: Option<alacritty_terminal::selection::SelectionRange>,
    palette: &AnsiColors,
    colors: &alacritty_terminal::term::color::Colors,
) -> CellView {
    let mut fg = resolve(colors, palette, cell.fg);
    let mut bg = resolve(colors, palette, cell.bg);
    if cell.flags.contains(Flags::INVERSE) {
        std::mem::swap(&mut fg, &mut bg);
    }
    let wide = cell.flags.contains(Flags::WIDE_CHAR);
    let selected = selection.is_some_and(|range| {
        range.contains(point) || (wide && range.contains(Point::new(point.line, point.column + 1)))
    });

    let mut text = String::new();
    if !cell.flags.contains(Flags::WIDE_CHAR_SPACER) && !cell.flags.contains(Flags::HIDDEN) {
        text.push(cell.c);
        if let Some(extras) = cell.zerowidth() {
            text.extend(extras.iter());
        }
    }

    CellView {
        text,
        style: CellStyle {
            fg,
            bg,
            bold: cell.flags.contains(Flags::BOLD),
            italic: cell.flags.contains(Flags::ITALIC),
            underline: cell
                .flags
                .intersects(Flags::UNDERLINE | Flags::DOUBLE_UNDERLINE),
            strikeout: cell.flags.contains(Flags::STRIKEOUT),
            hidden: cell.flags.contains(Flags::HIDDEN),
        },
        spacer: cell.flags.contains(Flags::WIDE_CHAR_SPACER),
        selected,
    }
}

/// OSC 设置过的颜色优先于主题表，这样 `ls --color` 里的自定义色不会被覆盖。
fn resolve(
    colors: &alacritty_terminal::term::color::Colors,
    palette: &AnsiColors,
    color: Color,
) -> u32 {
    match color {
        Color::Named(named) => colors[named as usize]
            .map(rgb_hex)
            .unwrap_or_else(|| palette.named(named)),
        Color::Indexed(index) => colors[index as usize]
            .map(rgb_hex)
            .unwrap_or_else(|| palette.indexed(index)),
        Color::Spec(Rgb { r, g, b }) => rgb_hex(Rgb { r, g, b }),
    }
}

fn rgb_hex(rgb: Rgb) -> u32 {
    ((rgb.r as u32) << 16) | ((rgb.g as u32) << 8) | rgb.b as u32
}

/// 离线解析：没有 PTY 时也能喂字节并读网格，测试与 ANSI 样例比对都靠它。
#[cfg(test)]
pub(crate) struct OfflineScreen {
    pub(crate) term: Term<crate::io::vt::VtListener>,
    pub(crate) parser: alacritty_terminal::vte::ansi::Processor,
}

#[cfg(test)]
impl OfflineScreen {
    pub(crate) fn new(
        size: super::event::ScreenSize,
        sender: async_channel::Sender<(u64, super::event::VtEvent)>,
        session: u64,
    ) -> Self {
        let config = alacritty_terminal::term::Config {
            scrolling_history: size.history,
            ..Default::default()
        };
        Self {
            term: Term::new(
                config,
                &size,
                crate::io::vt::VtListener::new(sender, session),
            ),
            parser: alacritty_terminal::vte::ansi::Processor::new(),
        }
    }

    pub(crate) fn feed(&mut self, bytes: &[u8]) {
        self.parser.advance(&mut self.term, bytes);
    }

    pub(crate) fn frame(&self, palette: &AnsiColors) -> Frame {
        frame(&self.term, palette)
    }

    pub(crate) fn mode(&self) -> alacritty_terminal::term::TermMode {
        *self.term.mode()
    }
}
