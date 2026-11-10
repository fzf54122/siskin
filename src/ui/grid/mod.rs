// @Time    : 2026/11/5 17:20
// @Author  : fzf
// @FileName: mod.rs
// @Software: Siskin

//! 终端网格渲染。这是 GPUI 里唯一手写 `Element` 的地方：
//!
//! - `prepaint` 把可视区每一行的 cell 聚成同色 run，一次整形一整行；
//! - `paint` 先铺背景矩形，再画文本，最后画光标；
//! - 宽字符按 `WIDE_CHAR_SPACER` 判定，不从字符宽度猜。
//!
//! 只整形可视行：滚动过的历史行不参与，这是 10 万行 scrollback 下的前提。

mod cache;
mod cursor;
mod element;
mod inputs;
mod paint;
mod runs;
mod shape;

pub(crate) use inputs::GridInputs;

use std::sync::{Arc, Mutex};

use gpui::{Bounds, IntoElement, Pixels, Point, SharedString, Size, Window, point, px, size};

use crate::domain::RenderMetrics;
use crate::io::search::MatchSpan;
use crate::io::vt::{CellStyle, Frame};

use runs::{Run, runs_for_row};

pub(crate) struct RowLayout {
    runs: Vec<(Point<Pixels>, Arc<gpui::ShapedLine>)>,
    backgrounds: Vec<(Point<Pixels>, Size<Pixels>, u32)>,
    /// OSC 8 链接段的底线。另外在 hover 时由 UI 层改指针形状。
    underlines: Vec<(Point<Pixels>, Pixels)>,
}

/// 按比例把 `accent` 混进 `base`，用于「淡一点的同类色」。
fn mix(base: u32, accent: u32, amount: f32) -> u32 {
    let amount = amount.clamp(0., 1.);
    let channel = |shift: u32| {
        let a = ((base >> shift) & 0xff) as f32;
        let b = ((accent >> shift) & 0xff) as f32;
        ((a * (1. - amount) + b * amount).round() as u32).min(0xff)
    };
    (channel(16) << 16) | (channel(8) << 8) | channel(0)
}

fn to_hsla(color: u32) -> gpui::Hsla {
    gpui::rgba((color << 8) | 0xff).into()
}

/// 光标矩形由渲染线程写入、应用层读取，只用于输入法候选框定位。
pub(crate) type CursorSlot = Arc<Mutex<Option<Bounds<Pixels>>>>;

/// 网格左上角在窗口坐标里的位置，鼠标换算要用；渲染时由 prepaint 写入。
pub(crate) type OriginSlot = Arc<Mutex<Option<Point<Pixels>>>>;

pub(crate) struct TerminalGrid {
    frame: Frame,
    /// 亚行平移。整帧同向移动，所以 prepaint 里统一加到原点上。
    offset_y: f32,
    metrics: RenderMetrics,
    font: SharedString,
    palette_bg: u32,
    palette_fg: u32,
    selection_bg: u32,
    /// 非当前匹配的底色。
    highlight_bg: u32,
    /// 当前匹配的底色。
    match_bg: u32,
    cursor_color: u32,
    cursor_focused: bool,
    /// OSC 8 链接的底线颜色。用强调色，与文字和搜索高亮都区分开。
    link_color: u32,
    /// 搜索匹配区间。高亮只改背景色，不改文字内容。
    highlights: Vec<MatchSpan>,
    current_match: Option<MatchSpan>,
    cursor_slot: CursorSlot,
    origin_slot: OriginSlot,
    view: gpui::Entity<crate::app::SiskinApp>,
    focus: gpui::FocusHandle,
    debug_id: Option<&'static str>,
}

impl TerminalGrid {
    pub(crate) fn new(inputs: GridInputs) -> Self {
        Self {
            frame: inputs.frame,
            offset_y: inputs.offset_y,
            metrics: inputs.metrics,
            font: inputs.font,
            palette_bg: inputs.palette_bg,
            palette_fg: inputs.palette_fg,
            selection_bg: inputs.selection_bg,
            highlight_bg: inputs.highlight_bg,
            match_bg: inputs.match_bg,
            cursor_color: inputs.cursor,
            cursor_focused: inputs.focused,
            link_color: inputs.link,
            highlights: inputs.highlights,
            current_match: inputs.current_match,
            cursor_slot: inputs.cursor_slot,
            origin_slot: inputs.origin_slot,
            view: inputs.view,
            focus: inputs.focus,
            debug_id: None,
        }
    }

    pub(crate) fn debug_selector(mut self, id: &'static str) -> Self {
        self.debug_id = Some(id);
        self
    }

    fn cell_origin(&self, origin: Point<Pixels>, row: usize, column: usize) -> Point<Pixels> {
        origin
            + point(
                px(column as f32 * self.metrics.cell_width),
                px(row as f32 * self.metrics.row_height),
            )
    }

    fn shape_run(&self, run: &Run, window: &mut Window) -> Arc<gpui::ShapedLine> {
        self.shape_cell(&run.text, run.style, window)
    }

    /// 单个单元格文本。用于推进量不可信的图标/emoji。
    fn shape_cell(
        &self,
        text: &str,
        style: CellStyle,
        window: &mut Window,
    ) -> Arc<gpui::ShapedLine> {
        cache::shape_cached(&self.font, text, style, self.metrics.font_size, window)
    }

    fn background_span(
        &self,
        run: &Run,
        row: usize,
        origin: Point<Pixels>,
    ) -> (Point<Pixels>, Size<Pixels>, u32) {
        (
            self.cell_origin(origin, row, run.start_column),
            size(
                px(run.cell_count as f32 * self.metrics.cell_width),
                px(self.metrics.row_height),
            ),
            run.style.bg,
        )
    }

    fn shape_row(&self, row: usize, origin: Point<Pixels>, window: &mut Window) -> RowLayout {
        let Some(line) = self.frame.rows.get(row) else {
            return RowLayout {
                runs: Vec::new(),
                backgrounds: Vec::new(),
                underlines: Vec::new(),
            };
        };
        let mut shaped = Vec::new();
        let mut backgrounds = Vec::new();
        let mut underlines = Vec::new();
        for run in runs_for_row(line) {
            if run.is_link() {
                // OSC 8 自带 SGR 时已有下划线；仅补纯文本链接缺失的那一种。
                if !run.style.underline {
                    underlines.push((
                        self.cell_origin(origin, row, run.start_column),
                        px(run.cell_count as f32 * self.metrics.cell_width),
                    ));
                }
            }
            if run.style.bg != self.palette_bg {
                backgrounds.push(self.background_span(&run, row, origin));
            }
            if run.needs_cell_positioning() {
                // 图标/emoji 的推进量不等于单元格宽度，逐格定位避免整行漂移。
                let mut column = run.start_column;
                for cell in line
                    .cells
                    .iter()
                    .skip(run.start_column)
                    .take(run.cell_count)
                {
                    if cell.spacer {
                        column += 1;
                        continue;
                    }
                    let position = self.cell_origin(origin, row, column);
                    shaped.push((position, self.shape_cell(&cell.text, run.style, window)));
                    column += 1;
                }
            } else {
                let position = self.cell_origin(origin, row, run.start_column);
                shaped.push((position, self.shape_run(&run, window)));
            }
        }
        backgrounds.extend(self.selection_spans(row, line, origin));
        backgrounds.extend(self.highlight_spans(row, origin));
        RowLayout {
            runs: shaped,
            backgrounds,
            underlines,
        }
    }

    /// 选区背景。相邻且同为选区底色的格子合成一个矩形，
    /// 一次绘制任务覆盖整段选区，而不是每格一个矩形。
    fn selection_spans(
        &self,
        row: usize,
        line: &crate::io::vt::RowView,
        origin: Point<Pixels>,
    ) -> Vec<(Point<Pixels>, Size<Pixels>, u32)> {
        selection_ranges(line)
            .into_iter()
            .map(|(start, end)| self.selection_span(row, start, end, origin))
            .collect()
    }

    /// 一段连续选区的矩形。`end` 是右开列。
    fn selection_span(
        &self,
        row: usize,
        start: usize,
        end: usize,
        origin: Point<Pixels>,
    ) -> (Point<Pixels>, Size<Pixels>, u32) {
        let width = (end.saturating_sub(start).max(1)) as f32;
        (
            self.cell_origin(origin, row, start),
            size(
                px(width * self.metrics.cell_width),
                px(self.metrics.row_height),
            ),
            self.selection_bg,
        )
    }

    /// 搜索高亮：普通匹配用半透明强调色，当前匹配用更实的底色。
    /// 网格只拿到「行 + 列区间」，不知道查询词本身。
    fn highlight_spans(
        &self,
        row: usize,
        origin: Point<Pixels>,
    ) -> Vec<(Point<Pixels>, Size<Pixels>, u32)> {
        let mut spans = Vec::new();
        for span in &self.highlights {
            if span.row != row {
                continue;
            }
            let is_current = self.current_match.as_ref() == Some(span);
            // 当前匹配用强调色，其它匹配用更淡的底色，避免整屏发黄。
            let color = if is_current {
                self.match_bg
            } else {
                mix(self.highlight_bg, self.palette_bg, 0.72)
            };
            let width = span.end.saturating_sub(span.start).max(1);
            spans.push((
                self.cell_origin(origin, row, span.start),
                size(
                    px(width as f32 * self.metrics.cell_width),
                    px(self.metrics.row_height),
                ),
                color,
            ));
        }
        spans
    }

    /// 光标矩形给输入法定位候选框；网格原点给鼠标换算行列。
    fn store_cursor(&self, origin: Point<Pixels>, element_origin: Point<Pixels>) {
        let cursor = self.frame.cursor;
        if let Ok(mut slot) = self.cursor_slot.lock() {
            *slot = Some(Bounds {
                origin: self.cell_origin(origin, cursor.row, cursor.column),
                size: size(px(self.metrics.cell_width), px(self.metrics.row_height)),
            });
        }
        if let Ok(mut slot) = self.origin_slot.lock() {
            *slot = Some(element_origin);
        }
    }
}

impl IntoElement for TerminalGrid {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// 一行的连续选区列区间。相邻的选中格子合成一段，`end` 右开。
/// 合并让一次绘制任务覆盖整段选区，80 列全选从 80 个矩形降到 1 个。
pub(crate) fn selection_ranges(line: &crate::io::vt::RowView) -> Vec<(usize, usize)> {
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    let mut run: Option<(usize, usize)> = None;
    for (column, cell) in line.cells.iter().enumerate() {
        if cell.selected {
            run = Some(match run {
                Some((start, _)) => (start, column),
                None => (column, column),
            });
            continue;
        }
        if let Some((start, end)) = run.take() {
            ranges.push((start, end + 1));
        }
    }
    if let Some((start, end)) = run.take() {
        ranges.push((start, end + 1));
    }
    ranges
}

#[cfg(test)]
mod tests {
    use super::selection_ranges;
    use crate::io::vt::{CellStyle, CellView, RowView};

    fn cell(selected: bool) -> CellView {
        CellView {
            text: "x".into(),
            style: CellStyle::default(),
            spacer: false,
            selected,
            link: None,
        }
    }

    fn row(selected: &[bool]) -> RowView {
        RowView {
            cells: selected.iter().map(|flag| cell(*flag)).collect(),
        }
    }

    #[test]
    fn contiguous_selected_cells_become_a_single_range() {
        // 8 列全选：一段。合并前这是 8 个绘制任务。
        assert_eq!(selection_ranges(&row(&[true; 8])), vec![(0, 8)]);
        // 中间断开：两段，[0,2) 与 [3,5)。
        assert_eq!(
            selection_ranges(&row(&[true, true, false, true, true])),
            vec![(0, 2), (3, 5)]
        );
        // 行首没有选区：第一段从第 1 列开始。
        assert_eq!(selection_ranges(&row(&[false, true, true])), vec![(1, 3)]);
        // 选区顶到行尾：尾段必须闭合，否则最后一列画不出来。
        assert_eq!(
            selection_ranges(&row(&[false, true, true, true])),
            vec![(1, 4)]
        );
        // 全不选：一段都没有。
        assert_eq!(selection_ranges(&row(&[false; 5])), Vec::new());
        // 空行：无段。
        assert_eq!(selection_ranges(&RowView { cells: Vec::new() }), Vec::new());
    }
}
