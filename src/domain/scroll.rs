// @Time    : 2026/11/9 12:20
// @Author  : fzf
// @FileName: scroll.rs
// @Software: Siskin

//! 平滑滚动状态。底层 `Scroll::Delta(i32)` 只接受整行，硬把滚轮像素量
//! 取整就会一卡一跳。这里把位移按像素累积，够一整行时才提交给终端，
//! 不足一行的余量交给渲染层做亚行平移，视觉上就是连续的。

/// 一个会话的滚动位置。`pending` 是尚未凑满一行的像素位移。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct ScrollOffset {
    /// 已提交给终端的整行数，向下为正。
    pub lines: i32,
    /// 不足一行的像素余量，符号与滚动方向一致。
    pub pending: f32,
}

impl ScrollOffset {
    /// 追加一次滚轮位移，返回应该提交给终端的整行数。
    /// 返回 0 表示还没攒够一行，只更新余量。
    pub(crate) fn accumulate(&mut self, pixels: f32, row_height: f32) -> i32 {
        if row_height <= 0. || pixels == 0. {
            return 0;
        }
        self.pending += pixels;
        let whole = (self.pending / row_height).trunc() as i32;
        if whole == 0 {
            return 0;
        }
        // 只提交整行，余数留在 `pending` 里继续攒。
        self.pending -= whole as f32 * row_height;
        self.lines += whole;
        whole
    }

    /// 当前亚行偏移量（像素）。渲染层把它加到网格原点上，
    /// 鼠标换算时要用同一个值做补偿，否则点击会错一行。
    pub(crate) fn sub_pixel(&self, row_height: f32) -> f32 {
        self.pending.min(row_height).max(-row_height)
    }

    /// 回到最新输出时清空余量：贴底不该留下半个行的偏移。
    pub(crate) fn snap_to_bottom(&mut self) {
        self.lines = 0;
        self.pending = 0.;
    }

    /// 滚动条跳转时只清亚行余量：指定行就是指定行，不留半行偏移，
    /// 但已提交的行数不动，它与 PTY 自己的偏移无关。
    pub(crate) fn clear_pending(&mut self) {
        self.pending = 0.;
    }

    /// 跳到滚动区顶部。余量同样清零，顶格就是顶格。
    pub(crate) fn snap_to_top(&mut self) {
        self.lines = i32::MAX;
        self.pending = 0.;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROW: f32 = 20.;

    #[test]
    fn a_smaller_than_row_delta_only_accumulates() {
        let mut offset = ScrollOffset::default();
        assert_eq!(offset.accumulate(7., ROW), 0);
        assert_eq!(offset.pending, 7.);
        // 还差 13px 才够一行，继续攒。
        assert_eq!(offset.accumulate(6., ROW), 0);
        assert_eq!(offset.pending, 13.);
        assert_eq!(offset.accumulate(7., ROW), 1);
        assert_eq!(offset.pending, 0.);
        assert_eq!(offset.lines, 1);
    }

    #[test]
    fn a_large_delta_commits_whole_rows_and_keeps_the_remainder() {
        let mut offset = ScrollOffset::default();
        // 3.4 行：提交 3 行，余 0.4 行 = 8px。
        assert_eq!(offset.accumulate(68., ROW), 3);
        assert_eq!(offset.pending, 8.);
        assert_eq!(offset.lines, 3);
    }

    #[test]
    fn opposite_directions_cancel_before_committing() {
        let mut offset = ScrollOffset::default();
        assert_eq!(offset.accumulate(-5., ROW), 0);
        assert_eq!(offset.accumulate(-6., ROW), 0);
        // 已经 -11px，回到 +11px 应该互相抵消，一行都不提交。
        assert_eq!(offset.accumulate(11., ROW), 0);
        assert_eq!(offset.pending, 0.);
        assert_eq!(offset.lines, 0);
    }

    #[test]
    fn sub_pixel_is_clamped_to_one_row() {
        let mut offset = ScrollOffset::default();
        offset.accumulate(-68., ROW);
        assert_eq!(offset.lines, -3);
        // 余量超出 0.4 行也不该让渲染层偏移超过一格。
        assert_eq!(offset.sub_pixel(ROW), -8.);
    }

    #[test]
    fn snapping_clears_both_the_committed_lines_and_the_remainder() {
        let mut offset = ScrollOffset::default();
        offset.accumulate(48., ROW);
        offset.snap_to_bottom();
        assert_eq!(offset.lines, 0);
        assert_eq!(offset.pending, 0.);
        assert_eq!(offset.sub_pixel(ROW), 0.);
    }

    #[test]
    fn a_zero_row_height_is_ignored_instead_of_dividing_by_zero() {
        let mut offset = ScrollOffset::default();
        assert_eq!(offset.accumulate(20., 0.), 0);
        assert_eq!(offset.sub_pixel(0.), 0.);
    }
}
