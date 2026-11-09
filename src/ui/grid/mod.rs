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

mod cursor;
mod inputs;
mod runs;
mod shape;

pub(crate) use inputs::GridInputs;

use std::sync::{Arc, Mutex};

use gpui::{
    App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement, LayoutId,
    Pixels, Point, SharedString, Size, Style, TextAlign, Window, fill, point, px, relative, size,
};

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
        shape::shape_text(&self.font, text, style, self.metrics.font_size, window)
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

    /// 选区用逐格背景矩形表达，跨 run 也连续。
    fn selection_spans(
        &self,
        row: usize,
        line: &crate::io::vt::RowView,
        origin: Point<Pixels>,
    ) -> Vec<(Point<Pixels>, Size<Pixels>, u32)> {
        line.cells
            .iter()
            .enumerate()
            .filter(|(_, cell)| cell.selected)
            .map(|(column, _)| {
                (
                    self.cell_origin(origin, row, column),
                    size(px(self.metrics.cell_width), px(self.metrics.row_height)),
                    self.selection_bg,
                )
            })
            .collect()
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

impl Element for TerminalGrid {
    type RequestLayoutState = ();
    type PrepaintState = Vec<RowLayout>;

    fn id(&self) -> Option<ElementId> {
        self.debug_id.map(|id| ElementId::from(id.to_string()))
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();
        (window.request_layout(style, None, cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        _cx: &mut App,
    ) -> Self::PrepaintState {
        let origin = bounds.origin + point(px(self.metrics.padding), px(self.metrics.padding));
        let visible_rows = (((bounds.size.height.as_f32() - self.metrics.padding * 2.)
            / self.metrics.row_height)
            .floor() as usize)
            .max(1)
            + 1;
        let layouts = (0..self.frame.rows.len().min(visible_rows))
            .map(|row| self.shape_row(row, origin, window))
            .collect();
        self.store_cursor(origin, bounds.origin);
        layouts
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        layouts: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        window.paint_quad(fill(bounds, to_hsla(self.palette_bg)));
        for row in layouts.iter() {
            for (position, extent, color) in &row.backgrounds {
                window.paint_quad(fill(Bounds::new(*position, *extent), to_hsla(*color)));
            }
            for (position, line) in &row.runs {
                let _ = line.paint(
                    *position,
                    px(self.metrics.row_height),
                    TextAlign::Left,
                    None,
                    window,
                    cx,
                );
            }
            // OSC 8 链接底线。画在单元格底部而不是贴基线的位置，
            // 这样和程序自己用 SGR 4 画的下划线观感一致。
            for (position, width) in &row.underlines {
                let baseline = position.y + px(self.metrics.row_height) - px(2.);
                window.paint_quad(fill(
                    Bounds::new(point(position.x, baseline), size(*width, px(1.))),
                    to_hsla(self.link_color),
                ));
            }
        }
        // 输入法通过这个 handler 拿到预编辑串与候选框锚点；必须在 paint 阶段注册。
        window.handle_input(
            &self.focus,
            gpui::ElementInputHandler::new(bounds, self.view.clone()),
            cx,
        );
        cursor::paint_cursor(
            self.frame.cursor,
            self.metrics,
            bounds.origin + point(px(self.metrics.padding), px(self.metrics.padding)),
            self.cursor_color,
            self.palette_fg,
            self.cursor_focused,
            window,
        );
    }
}
