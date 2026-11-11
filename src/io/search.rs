// @Time    : 2026/11/5 22:00
// @Author  : fzf
// @FileName: search.rs
// @Software: Siskin

//! 搜索的共享小件：可见行匹配区间（`MatchSpan`，渲染层按它画高亮）
//! 与「下一个匹配」的循环下标。实际搜索（字面 / 正则 / 大小写、
//! 完整 scrollback）见 `history_search`。

/// 一个匹配区间。`row` 是可见行下标，列区间左闭右开。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MatchSpan {
    pub row: usize,
    pub start: usize,
    pub end: usize,
}

/// 「下一个匹配」的循环下标。没有匹配时返回 `None`。
pub(crate) fn next_index(current: Option<usize>, count: usize, forward: bool) -> Option<usize> {
    if count == 0 {
        return None;
    }
    match current {
        None => Some(if forward { 0 } else { count - 1 }),
        Some(index) if forward => Some((index + 1) % count),
        Some(index) => Some((index + count - 1) % count),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_index_wraps_in_both_directions() {
        assert_eq!(next_index(None, 3, true), Some(0));
        assert_eq!(next_index(None, 3, false), Some(2));
        assert_eq!(next_index(Some(2), 3, true), Some(0));
        assert_eq!(next_index(Some(0), 3, false), Some(2));
        assert_eq!(next_index(Some(1), 0, true), None);
    }
}
