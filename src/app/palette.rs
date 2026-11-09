// @Time    : 2026/11/5 12:22
// @Author  : fzf
// @FileName: palette.rs
// @Software: Siskin

//! 命令面板状态。只保存查询串与选中项，过滤和执行分别由
//! `app/mod.rs` 与 `Action` 负责。

use super::actions::PaletteEntry;

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

    pub fn matches(&self, entry: &PaletteEntry) -> bool {
        let query = self.query.to_lowercase();
        query.is_empty()
            || entry.label.to_lowercase().contains(&query)
            || entry.keywords.contains(&query)
            || entry.keys.to_lowercase().contains(&query)
            || entry.hint.to_lowercase().contains(&query)
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
