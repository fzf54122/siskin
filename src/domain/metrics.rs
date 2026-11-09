// @Time    : 2026/11/5 15:15
// @Author  : fzf
// @FileName: metrics.rs
// @Software: Siskin

//! 字体度量与行列换算。终端网格必须由真实字形宽度推导，不能用 `0.6 em`
//! 这类魔数，否则宽字符与连字会逐行累积错位。

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RenderMetrics {
    pub cell_width: f32,
    pub row_height: f32,
    pub font_size: f32,
    /// 网格周围的留白，行数列数按扣除留白后的区域计算。
    pub padding: f32,
}

impl RenderMetrics {
    pub(crate) fn new(cell_width: f32, row_height: f32, font_size: f32, padding: f32) -> Self {
        Self {
            cell_width: cell_width.max(1.),
            row_height: row_height.max(font_size * 1.1),
            font_size,
            padding,
        }
    }

    /// 可用区域能容纳的列数，至少 2 列，否则网格无法工作。
    pub(crate) fn columns(&self, width: f32) -> usize {
        (((width - self.padding * 2.) / self.cell_width).floor() as usize).max(2)
    }

    pub(crate) fn rows(&self, height: f32) -> usize {
        (((height - self.padding * 2.) / self.row_height).floor() as usize).max(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_and_rows_are_floored_to_whole_cells() {
        let metrics = RenderMetrics::new(8., 18., 13., 8.);
        assert_eq!(metrics.columns(8. * 10. + 16. + 7.), 10);
        assert_eq!(metrics.rows(18. * 5. + 16. + 17.), 5);
    }

    #[test]
    fn tiny_areas_still_produce_a_usable_grid() {
        let metrics = RenderMetrics::new(8., 18., 13., 8.);
        assert_eq!(metrics.columns(4.), 2);
        assert_eq!(metrics.rows(4.), 1);
    }

    #[test]
    fn invalid_cell_width_is_clamped() {
        let metrics = RenderMetrics::new(0., 10., 13., 0.);
        assert!(metrics.cell_width >= 1.);
    }
}
