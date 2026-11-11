// @Time    : 2026/11/5 12:22
// @Author  : fzf
// @FileName: palette.rs
// @Software: Siskin

//! 命令面板状态。只保存查询串与选中项，过滤和排序由这里负责，
//! 执行由 `app/mod.rs` 与 `Action` 负责。

use super::actions::PaletteEntry;
use super::fuzzy::fuzzy_match;
use crate::domain::Action;

#[derive(Clone, Debug, Default)]
pub(crate) struct CommandPalette {
    pub query: String,
    pub selected: usize,
}

impl CommandPalette {
    pub fn type_char(&mut self, ch: char) {
        if !ch.is_control() {
            self.query.push(ch);
            self.selected = 0;
        }
    }

    pub fn backspace(&mut self) {
        self.query.pop();
        self.selected = 0;
    }

    /// 条目的最高相关分。label 命中权重最高，其次是 keywords、
    /// keys、hint；空查询返回 0（全部展示，由 `ranked` 排最近优先）。
    pub fn score(&self, entry: &PaletteEntry) -> Option<i32> {
        let query = self.query.trim();
        if query.is_empty() {
            return Some(0);
        }
        let label = fuzzy_match(query, entry.label).map(|m| m.score * 2);
        let keywords = fuzzy_match(query, entry.keywords).map(|m| m.score);
        let keys = fuzzy_match(query, entry.keys).map(|m| m.score);
        let hint = fuzzy_match(query, entry.hint).map(|m| m.score);
        [label, keywords, keys, hint].into_iter().flatten().max()
    }

    /// 查询在 label 里的命中区间（字节偏移），供 UI 高亮；
    /// label 没命中时为空，UI 就不画高亮。
    pub fn label_ranges(&self, entry: &PaletteEntry) -> Vec<(usize, usize)> {
        fuzzy_match(self.query.trim(), entry.label)
            .map(|m| m.ranges)
            .unwrap_or_default()
    }

    /// 排好序的条目下标（下标指向 `PALETTE_ENTRIES`），附带分数：
    /// 空查询时最近操作在前、其余按原顺序；有查询时按模糊分从高到低，
    /// 最近操作加 20 分置前加权，同分保持原顺序。
    pub fn ranked(&self, recent: &[Action]) -> Vec<(usize, i32)> {
        let empty = self.query.trim().is_empty();
        let mut ranked: Vec<(usize, i32, usize)> = super::actions::PALETTE_ENTRIES
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| {
                let mut score = self.score(entry)?;
                let recent_rank = recent
                    .iter()
                    .position(|action| *action == entry.action)
                    .unwrap_or(usize::MAX);
                if recent_rank != usize::MAX {
                    score += 20;
                }
                Some((index, score, recent_rank))
            })
            .collect();
        ranked.sort_by(|a, b| {
            if empty {
                // 空查询：最近在前，同级按原顺序。
                a.2.cmp(&b.2).then(a.0.cmp(&b.0))
            } else {
                b.1.cmp(&a.1).then(a.0.cmp(&b.0))
            }
        });
        ranked
            .into_iter()
            .map(|(index, score, _)| (index, score))
            .collect()
    }

    pub fn move_selection(&mut self, down: bool, count: usize) {
        if count == 0 {
            return;
        }
        self.selected = if down {
            (self.selected + 1).min(count - 1)
        } else {
            self.selected.saturating_sub(1)
        };
    }
}
