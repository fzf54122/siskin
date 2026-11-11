// @Time    : 2026/11/11
// @Author  : fzf
// @FileName: pane.rs
// @Software: Siskin

//! 分屏布局树。单窗口内的二叉布局，不引入通用布局框架。
//!
//! - 一个标签（`Session`）就是一棵树，叶指向会话 id（不是下标），
//!   会话增删、标签切换都不会让叶串线；
//! - `Horizontal` 是横向分隔线、上下叠放，`Vertical` 是纵向分隔线、
//!   左右并排；比例按第一个子树的占比计，钳在 0.1..=0.9；
//! - 纯数据结构、无 GPUI 依赖（布局矩形只用 `Bounds` 做载体），
//!   拖动、派生、关闭都在 `pane_ops.rs` 的应用方法里。

use gpui::{Bounds, Pixels, point, px, size};

/// 分隔线厚度（像素）。布局与渲染共用，改一处两边都变。
pub(crate) const PANE_DIVIDER: f32 = 4.;

fn clamp_ratio(ratio: f32) -> f32 {
    ratio.clamp(0.1, 0.9)
}

/// 分隔线拖动中的状态。只记拖的是哪条线（用第二个子树的首叶 id
/// 标识，分隔线与该 id 一一对应）、起点与起始比例，移动时按
/// 位移 / 总长重算比例；与文本选区的 `drag` 是两个字段，不共用。
#[derive(Clone, Copy, Debug)]
pub(crate) struct PaneDrag {
    pub second_leaf: u64,
    pub horizontal: bool,
    pub start: f32,
    pub ratio: f32,
    pub total: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SplitDirection {
    /// 横向分隔线：上下叠放，拖动改高度占比。
    Horizontal,
    /// 纵向分隔线：左右并排，拖动改宽度占比。
    Vertical,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum PaneTree {
    Leaf(u64),
    Split {
        direction: SplitDirection,
        ratio: f32,
        first: Box<PaneTree>,
        second: Box<PaneTree>,
    },
}

impl PaneTree {
    pub(crate) fn leaf(id: u64) -> Self {
        PaneTree::Leaf(id)
    }

    pub(crate) fn leaf_ids(&self) -> Vec<u64> {
        match self {
            PaneTree::Leaf(id) => vec![*id],
            PaneTree::Split { first, second, .. } => {
                let mut ids = first.leaf_ids();
                ids.extend(second.leaf_ids());
                ids
            }
        }
    }

    pub(crate) fn contains(&self, id: u64) -> bool {
        match self {
            PaneTree::Leaf(leaf) => *leaf == id,
            PaneTree::Split { first, second, .. } => first.contains(id) || second.contains(id),
        }
    }

    pub(crate) fn first_leaf(&self) -> Option<u64> {
        match self {
            PaneTree::Leaf(id) => Some(*id),
            PaneTree::Split { first, .. } => first.first_leaf(),
        }
    }

    /// 把 `leaf_id` 处的叶换成一个二分节点：原叶在前，新叶在后，
    /// 初始比例 0.5。叶不存在时返回 `false`，树不动。
    pub(crate) fn split(&mut self, leaf_id: u64, direction: SplitDirection, new_id: u64) -> bool {
        match self {
            PaneTree::Leaf(id) if *id == leaf_id => {
                *self = PaneTree::Split {
                    direction,
                    ratio: 0.5,
                    first: Box::new(PaneTree::Leaf(leaf_id)),
                    second: Box::new(PaneTree::Leaf(new_id)),
                };
                true
            }
            PaneTree::Leaf(_) => false,
            PaneTree::Split { first, second, .. } => {
                first.split(leaf_id, direction, new_id) || second.split(leaf_id, direction, new_id)
            }
        }
    }

    /// 移除一个叶，兄弟子树顶替父节点的位置。只剩一个叶时
    /// 返回 `false`（最后一个叶由标签关闭处理，不在这里删）。
    pub(crate) fn close_leaf(&mut self, id: u64) -> bool {
        match self {
            PaneTree::Leaf(_) => false,
            PaneTree::Split { first, second, .. } => {
                if let PaneTree::Leaf(leaf) = first.as_ref()
                    && *leaf == id
                {
                    *self = (**second).clone();
                    return true;
                }
                if let PaneTree::Leaf(leaf) = second.as_ref()
                    && *leaf == id
                {
                    *self = (**first).clone();
                    return true;
                }
                first.close_leaf(id) || second.close_leaf(id)
            }
        }
    }

    /// 按叶序循环取下一个焦点；当前叶不在树里时取首叶。
    pub(crate) fn focus_next(&self, current: u64) -> Option<u64> {
        let ids = self.leaf_ids();
        if ids.is_empty() {
            return None;
        }
        let Some(at) = ids.iter().position(|id| *id == current) else {
            return ids.first().copied();
        };
        Some(ids[(at + 1) % ids.len()])
    }

    pub(crate) fn focus_prev(&self, current: u64) -> Option<u64> {
        let ids = self.leaf_ids();
        if ids.is_empty() {
            return None;
        }
        let Some(at) = ids.iter().position(|id| *id == current) else {
            return ids.last().copied();
        };
        Some(ids[(at + ids.len() - 1) % ids.len()])
    }

    /// 按「第二个子树的首叶」定位分隔线并改比例，钳在 0.1..=0.9。
    /// 找不到这条线时返回 `false`。
    pub(crate) fn set_ratio(&mut self, second_leaf: u64, ratio: f32) -> bool {
        match self {
            PaneTree::Leaf(_) => false,
            PaneTree::Split {
                ratio: current,
                second,
                ..
            } if second.first_leaf() == Some(second_leaf) => {
                *current = clamp_ratio(ratio);
                true
            }
            PaneTree::Split { first, second, .. } => {
                first.set_ratio(second_leaf, ratio) || second.set_ratio(second_leaf, ratio)
            }
        }
    }

    /// 找分隔线：方向、当前比例、第一个子树的首叶。
    /// 拖动起点需要这两个子树的叶 id 才能算出分隔线总长。
    pub(crate) fn split_info(&self, second_leaf: u64) -> Option<(SplitDirection, f32, u64)> {
        match self {
            PaneTree::Leaf(_) => None,
            PaneTree::Split {
                direction,
                ratio,
                first,
                second,
            } if second.first_leaf() == Some(second_leaf) => {
                Some((*direction, *ratio, first.first_leaf()?))
            }
            PaneTree::Split { first, second, .. } => first
                .split_info(second_leaf)
                .or_else(|| second.split_info(second_leaf)),
        }
    }

    /// 某个分隔线两侧子树的叶 id，用于按 bounds 并集算分隔线总长。
    pub(crate) fn split_leaves(&self, second_leaf: u64) -> Option<(Vec<u64>, Vec<u64>)> {
        match self {
            PaneTree::Leaf(_) => None,
            PaneTree::Split { first, second, .. } if second.first_leaf() == Some(second_leaf) => {
                Some((first.leaf_ids(), second.leaf_ids()))
            }
            PaneTree::Split { first, second, .. } => first
                .split_leaves(second_leaf)
                .or_else(|| second.split_leaves(second_leaf)),
        }
    }

    /// 把一块区域按比例切成两块，中间留出分隔线厚度。
    /// 布局（`resize_panes`）与渲染（`pane_node`）共用这一个切法，
    /// 两边算出的叶矩形不会漂移。
    pub(crate) fn split_rects(
        area: Bounds<Pixels>,
        direction: SplitDirection,
        ratio: f32,
    ) -> (Bounds<Pixels>, Bounds<Pixels>) {
        let ratio = clamp_ratio(ratio);
        match direction {
            SplitDirection::Vertical => {
                let total = (area.size.width.as_f32() - PANE_DIVIDER).max(0.);
                let first = total * ratio;
                let second = (total - first).max(0.);
                (
                    Bounds::new(area.origin, size(px(first), area.size.height)),
                    Bounds::new(
                        area.origin + point(px(first + PANE_DIVIDER), px(0.)),
                        size(px(second), area.size.height),
                    ),
                )
            }
            SplitDirection::Horizontal => {
                let total = (area.size.height.as_f32() - PANE_DIVIDER).max(0.);
                let first = total * ratio;
                let second = (total - first).max(0.);
                (
                    Bounds::new(area.origin, size(area.size.width, px(first))),
                    Bounds::new(
                        area.origin + point(px(0.), px(first + PANE_DIVIDER)),
                        size(area.size.width, px(second)),
                    ),
                )
            }
        }
    }

    /// 递归把区域分给每个叶。返回的矩形是窗口坐标，
    /// `resize_panes` 与 `pane_node` 都以它为准。
    pub(crate) fn layout(&self, area: Bounds<Pixels>) -> Vec<(u64, Bounds<Pixels>)> {
        match self {
            PaneTree::Leaf(id) => vec![(*id, area)],
            PaneTree::Split {
                direction,
                ratio,
                first,
                second,
            } => {
                let (first_rect, second_rect) = Self::split_rects(area, *direction, *ratio);
                let mut out = first.layout(first_rect);
                out.extend(second.layout(second_rect));
                out
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree() -> PaneTree {
        let mut tree = PaneTree::leaf(1);
        assert!(tree.split(1, SplitDirection::Vertical, 2));
        assert!(tree.split(2, SplitDirection::Horizontal, 3));
        tree
    }

    #[test]
    fn splitting_keeps_leaf_order_and_ids() {
        let tree = tree();
        assert_eq!(tree.leaf_ids(), vec![1, 2, 3]);
        assert!(tree.contains(2));
        assert!(!tree.contains(9));
        assert_eq!(tree.first_leaf(), Some(1));
    }

    #[test]
    fn splitting_an_unknown_leaf_leaves_the_tree_unchanged() {
        let mut tree = PaneTree::leaf(1);
        assert!(!tree.split(9, SplitDirection::Vertical, 2));
        assert_eq!(tree, PaneTree::leaf(1));
    }

    #[test]
    fn closing_a_leaf_promotes_its_sibling() {
        let mut tree = tree();
        assert!(tree.close_leaf(2));
        assert_eq!(tree.leaf_ids(), vec![1, 3]);
        assert!(tree.close_leaf(3));
        assert_eq!(tree, PaneTree::leaf(1));
    }

    #[test]
    fn the_last_leaf_is_never_closed_by_the_tree() {
        let mut tree = PaneTree::leaf(1);
        assert!(!tree.close_leaf(1));
        assert_eq!(tree, PaneTree::leaf(1));
        let empty = PaneTree::leaf(0);
        assert_eq!(empty.leaf_ids(), vec![0]);
    }

    #[test]
    fn focus_cycles_forward_and_backward() {
        let tree = tree();
        assert_eq!(tree.focus_next(1), Some(2));
        assert_eq!(tree.focus_next(3), Some(1));
        assert_eq!(tree.focus_prev(1), Some(3));
        assert_eq!(tree.focus_prev(2), Some(1));
        assert_eq!(tree.focus_next(99), Some(1));
    }

    #[test]
    fn ratios_are_clamped_to_the_usable_range() {
        let mut tree = tree();
        assert!(tree.set_ratio(2, 0.05));
        assert!(tree.set_ratio(3, 2.0));
        // 第二个子树的首叶是 2 的那条线被钳到 0.1，
        // 首叶是 3 的横向线被钳到 0.9。
        assert_eq!(tree.split_info(2).map(|info| info.1), Some(0.1));
        assert_eq!(tree.split_info(3).map(|info| info.1), Some(0.9));
        assert!(!tree.set_ratio(99, 0.5));
    }

    #[test]
    fn split_info_and_leaves_identify_each_divider() {
        let tree = tree();
        let (direction, ratio, first) = tree.split_info(2).unwrap();
        assert_eq!(direction, SplitDirection::Vertical);
        assert_eq!(ratio, 0.5);
        assert_eq!(first, 1);
        assert_eq!(tree.split_leaves(3), Some((vec![2], vec![3])));
        assert_eq!(tree.split_leaves(2), Some((vec![1], vec![2, 3])));
        assert!(tree.split_info(99).is_none());
    }

    #[test]
    fn layout_partitions_the_area_without_gaps() {
        let tree = tree();
        let area = Bounds::new(point(px(0.), px(28.)), size(px(1200.), px(772.)));
        let rects = tree.layout(area);
        assert_eq!(rects.len(), 3);
        let by_id = |id: u64| rects.iter().find(|(leaf, _)| *leaf == id).unwrap().1;
        let first = by_id(1);
        let second = by_id(2);
        let third = by_id(3);
        // 纵向先切：左叶占一半减分隔线，右半再横切成两块。
        assert!((first.size.width.as_f32() - 598.).abs() < 0.01);
        assert_eq!(second.size.width, first.size.width);
        assert_eq!(third.size.width, first.size.width);
        assert!((second.size.height.as_f32() - 384.).abs() < 0.01);
        assert!((third.size.height.as_f32() - 384.).abs() < 0.01);
        assert_eq!(first.origin, area.origin);
        assert_eq!(
            third.origin.y,
            second.origin.y + second.size.height + px(4.)
        );
    }
}
