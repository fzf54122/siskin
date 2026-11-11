// @Time    : 2026/11/11
// @Author  : fzf
// @FileName: ghost.rs
// @Software: Siskin

//! 行内自动补全（ghost text）的整形：光标行、光标列之后、弱化色。
//! 与 `paint.rs` / `cursor.rs` 一样单独成模块，`mod.rs` 不再长下去。

use std::sync::Arc;

use gpui::{Pixels, Point, Window};

use crate::io::vt::{CellStyle, RowView};

use super::TerminalGrid;

impl TerminalGrid {
    /// ghost 文本的整形位置：光标行、光标列之后、弱化色。
    /// 只在焦点 pane 且光标可见时画；按列截断到行尾，不越过网格右边界；
    /// 宽字符按自己的列宽推进，组合字符跟在基字同一格。
    pub(super) fn ghost_runs(
        &self,
        row: usize,
        line: &RowView,
        origin: Point<Pixels>,
        window: &mut Window,
    ) -> Vec<(Point<Pixels>, Arc<gpui::ShapedLine>)> {
        let Some(text) = self.ghost.as_deref() else {
            return Vec::new();
        };
        if !self.cursor_focused || !self.frame.cursor.visible || self.frame.cursor.row != row {
            return Vec::new();
        }
        let column = self.frame.cursor.column;
        let remaining = line.cells.len().saturating_sub(column);
        let text = crate::domain::ghost::truncate_to_columns(text, remaining);
        if text.is_empty() {
            return Vec::new();
        }
        let style = CellStyle {
            fg: self.ghost_color,
            bg: self.palette_bg,
            ..CellStyle::default()
        };
        if text.is_ascii() {
            // 一次整形整段，与普通 ASCII run 一样快。
            let position = self.cell_origin(origin, row, column);
            return vec![(position, self.shape_cell(text, style, window))];
        }
        let mut runs = Vec::new();
        let mut col = column;
        let mut cell_start = 0usize;
        let mut cell_width = 0usize;
        for (index, ch) in text.char_indices() {
            let width = crate::domain::ghost::char_width(ch);
            if width == 0 {
                continue;
            }
            if index != cell_start {
                let position = self.cell_origin(origin, row, col);
                runs.push((
                    position,
                    self.shape_cell(&text[cell_start..index], style, window),
                ));
                col += cell_width;
            }
            cell_start = index;
            cell_width = width;
        }
        if cell_start < text.len() {
            let position = self.cell_origin(origin, row, col);
            runs.push((
                position,
                self.shape_cell(&text[cell_start..], style, window),
            ));
        }
        runs
    }
}
