// @Time    : 2026/11/11
// @Author  : fzf
// @FileName: tab_ops.rs
// @Software: Siskin

//! 标签拖动排序与显式重命名。
//!
//! 拖拽选的是手动鼠标事件方案（b），不是 GPUI 的 `on_drag`：
//! 查过 vendored GPUI（rev c427327，`crates/gpui/src/elements/div.rs`），
//! `on_drag` 要求给拖拽物构造一个 `Entity<W: Render>` 拖拽视图，
//! 只为在同一根 tab 条内换位要多搭一套视图，不值；手动方案复用
//! 分隔线拖动已有的 `on_mouse_down` / `on_mouse_move` / `on_mouse_up`
//! 模式：按下记起始标签，移过别的标签时当场换位（live reorder），
//! 松开结束，被拖标签半透明。双击重命名走 `ClickEvent::click_count()`。
//!
//! 移动与改名都只改 `Sessions` 向量顺序与 `Session.custom_title`，
//! 不改会话 id：分屏叶按 id 引用，`active_pane` 与各标签的树都不受影响。
//! 持久化走 `mark_dirty` → 防抖落盘，与选会话、关会话同一条路，
//! 不另起通知通道。

use super::keys::KeyPress;
use super::{Session, Sessions, SiskinApp};
use crate::domain::text_edit::TextEdit;

/// 一次标签拖拽。`moved` 记拖动中是否真的换过位：
// 换过位才要在 mouse-up 后的 click 事件里吞掉那次点击，
// 否则松开就顺手把拖到的标签选了。
pub(crate) struct TabDrag {
    pub from: usize,
    pub moved: bool,
}

/// 正在内联重命名的标签。`edit` 是 `TextEdit`，与取色、字体输入框
/// 共用 `editor()`，于是输入法（`input_handler.rs`）与剪贴板
/// （`editor_clipboard`）不用再为重命名单写一份。
pub(crate) struct TabRename {
    pub index: usize,
    pub edit: TextEdit,
}

impl Session {
    /// 显式改名。`None` 清除显式名，`display_title` 恢复目录回退。
    pub(crate) fn rename(&mut self, name: Option<String>) {
        self.custom_title = name.filter(|name| !name.is_empty());
    }

    pub(crate) fn display_title(&self) -> String {
        // 显式名优先：哪怕是「终端」也照显，不再被目录名覆盖。
        if let Some(name) = self.custom_title.as_ref()
            && !name.is_empty()
        {
            return name.clone();
        }
        // 会话名优先跟随工作目录，`vim`/`ssh` 改 OSC 标题时标签保持稳定；
        // 目录名与显式会话名相同（如 `siskin`）时也一样。
        if let Some(name) = self.directory.as_ref().and_then(|path| path.file_name()) {
            let name = name.to_string_lossy();
            if self.title.as_ref() == name.as_ref()
                || self.title.as_ref() == "终端"
                || self.title.starts_with("会话 ")
            {
                return name.into_owned();
            }
        }
        self.title.to_string()
    }
}

impl Sessions {
    /// 把 `from` 处的会话移到 `to` 处。越界或原地移动是空操作，
    /// 返回 `false`，不报错。`active` 按移动后活动会话的 id 重算：
    /// 被移动的就是 active 时跟着它走，别的标签跨过 active 时
    /// 下标相应 ±1，其余情况不变。
    pub(crate) fn move_session(&mut self, from: usize, to: usize) -> bool {
        if from >= self.items.len() || to >= self.items.len() || from == to {
            return false;
        }
        let active_id = self.items[self.active].id;
        let item = self.items.remove(from);
        self.items.insert(to, item);
        self.active = self
            .items
            .iter()
            .position(|session| session.id == active_id)
            .unwrap_or(0);
        true
    }
}

impl SiskinApp {
    /// 标签拖拽的换位入口。顺序变了才记脏、才重算 pane 焦点
    /// （焦点按标签 id 记，`active_pane` 本身是会话 id，不受影响）。
    pub(crate) fn move_tab(&mut self, from: usize, to: usize) -> bool {
        if !self.sessions.move_session(from, to) {
            return false;
        }
        self.restore_pane_focus();
        self.session_scroll.scroll_to_item(self.active_index());
        self.mark_dirty();
        true
    }

    /// 标签上按下左键：记下起始标签；正在改别的标签时先提交它
    /// （失焦即提交），正在改本标签时不启动拖拽。
    pub(crate) fn start_tab_drag(&mut self, index: usize) {
        if self
            .tab_rename
            .as_ref()
            .is_some_and(|rename| rename.index == index)
        {
            return;
        }
        if self.tab_rename.is_some() {
            self.commit_tab_rename();
        }
        self.tab_click_suppressed = false;
        if index < self.sessions.len() {
            self.tab_drag = Some(TabDrag {
                from: index,
                moved: false,
            });
        }
    }

    /// 拖动中移过一个标签：当场换位，被拖标签跟到新下标。
    /// 只在按住左键时由 tab 的 `on_mouse_move` 调。
    pub(crate) fn hover_tab_drag(&mut self, index: usize) {
        let Some(from) = self.tab_drag.as_ref().map(|drag| drag.from) else {
            return;
        };
        if from == index {
            return;
        }
        if self.move_tab(from, index)
            && let Some(drag) = self.tab_drag.as_mut()
        {
            drag.from = index;
            drag.moved = true;
        }
    }

    /// 松开鼠标结束拖拽。换过位时记下「吞掉接下来那次 click」，
    /// 下次按下（`start_tab_drag`）或被消费（tab 的 `on_click`）时清掉。
    pub(crate) fn end_tab_drag(&mut self) {
        if let Some(drag) = self.tab_drag.take() {
            self.tab_click_suppressed = drag.moved;
        }
    }

    pub(crate) fn tab_dragging(&self, index: usize) -> bool {
        self.tab_drag
            .as_ref()
            .is_some_and(|drag| drag.from == index)
    }

    /// 双击或命令面板进入重命名。初始文本是当前显示的标题，
    /// 全选，方便直接敲新名覆盖。
    pub(crate) fn begin_tab_rename(&mut self, index: usize) {
        let Some(session) = self.sessions.get(index) else {
            return;
        };
        let mut edit = TextEdit::new(session.display_title());
        edit.select_all();
        self.tab_rename = Some(TabRename { index, edit });
        self.request_focus = true;
    }

    /// 命令面板入口：重命名当前（活动）标签。
    pub(crate) fn rename_active_tab(&mut self) {
        self.begin_tab_rename(self.active_index());
    }

    /// 回车提交：空串（去空白后）视为清除显式名，恢复目录回退。
    pub(crate) fn commit_tab_rename(&mut self) {
        let Some(rename) = self.tab_rename.take() else {
            return;
        };
        let name = rename.edit.text.trim().to_string();
        let name = if name.is_empty() { None } else { Some(name) };
        if let Some(session) = self.sessions.get_mut(rename.index) {
            session.rename(name);
        }
        self.mark_dirty();
        self.request_focus = true;
    }

    pub(crate) fn cancel_tab_rename(&mut self) {
        self.tab_rename = None;
        self.request_focus = true;
    }

    /// 重命名编辑框的按键：回车/Tab 提交，Esc 取消，其余是
    /// `TextEdit` 的常规编辑（方向键、Home/End、退格、删除、
    /// `Ctrl+A` 全选、字符输入）。调用方（`keys.rs` 的
    /// `rename_stage`）保证只有重命名打开时才进这里。
    pub(crate) fn rename_key(&mut self, press: &KeyPress<'_>) {
        match press.key {
            "escape" => {
                self.cancel_tab_rename();
                return;
            }
            "enter" | "tab" => {
                self.commit_tab_rename();
                return;
            }
            _ => {}
        }
        let Some(rename) = self.tab_rename.as_mut() else {
            return;
        };
        let edit = &mut rename.edit;
        match press.key {
            "a" if press.control || press.platform => edit.select_all(),
            "left" => edit.move_character(false, press.shift),
            "right" => edit.move_character(true, press.shift),
            "home" => edit.move_to(0, press.shift),
            "end" => edit.move_to(edit.text.len(), press.shift),
            "backspace" => edit.delete(true),
            "delete" => edit.delete(false),
            _ if !press.modified() => {
                if let Some(text) = press
                    .key_char
                    .or_else(|| (press.key.len() == 1).then_some(press.key))
                {
                    edit.replace(None, text, false);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::shell_spec::ShellKind;

    fn sessions(count: usize) -> Sessions {
        let mut set = Sessions::new(Session::starting(0, "第一个", ShellKind::Zsh));
        for index in 1..count {
            let id = set.next_id();
            set.add(Session::starting(
                id,
                format!("会话 {index}"),
                ShellKind::Zsh,
            ));
        }
        set
    }

    fn ids(set: &Sessions) -> Vec<u64> {
        set.iter().map(|(_, session)| session.id).collect()
    }

    #[test]
    fn moving_the_active_tab_carries_the_active_index_with_it() {
        let mut set = sessions(4);
        set.select(1);
        assert!(set.move_session(1, 3));
        assert_eq!(ids(&set), vec![0, 2, 3, 1]);
        assert_eq!(set.active_index(), 3);
        assert_eq!(set.active().id, 1);
    }

    #[test]
    fn moving_a_tab_across_the_active_one_shifts_the_index() {
        // 前面的标签跨过 active 移到后面：active 下标 -1。
        let mut set = sessions(4);
        set.select(1);
        assert!(set.move_session(0, 3));
        assert_eq!(ids(&set), vec![1, 2, 3, 0]);
        assert_eq!(set.active_index(), 0);
        // 后面的标签跨过 active 移到前面：active 下标 +1。
        let mut set = sessions(4);
        set.select(2);
        assert!(set.move_session(3, 0));
        assert_eq!(ids(&set), vec![3, 0, 1, 2]);
        assert_eq!(set.active_index(), 3);
    }

    #[test]
    fn moving_to_either_end_keeps_other_tabs_in_order() {
        let mut set = sessions(4);
        set.select(2);
        assert!(set.move_session(2, 0));
        assert_eq!(ids(&set), vec![2, 0, 1, 3]);
        assert_eq!(set.active_index(), 0);
        assert!(set.move_session(0, 3));
        assert_eq!(ids(&set), vec![0, 1, 3, 2]);
        assert_eq!(set.active_index(), 3);
    }

    #[test]
    fn out_of_range_and_in_place_moves_are_no_ops() {
        let mut set = sessions(3);
        set.select(1);
        assert!(!set.move_session(9, 0));
        assert!(!set.move_session(0, 9));
        assert!(!set.move_session(0, 3));
        assert!(!set.move_session(1, 1));
        assert_eq!(ids(&set), vec![0, 1, 2]);
        assert_eq!(set.active_index(), 1);
    }

    #[test]
    fn moving_a_tab_does_not_change_session_ids() {
        let mut set = sessions(3);
        assert!(set.move_session(0, 2));
        assert_eq!(ids(&set), vec![1, 2, 0]);
        assert_eq!(set.position(0), Some(2));
        assert_eq!(set.position(1), Some(0));
        assert_eq!(set.position(2), Some(1));
    }

    #[test]
    fn display_title_falls_back_to_the_directory_name() {
        let mut session = Session::starting(0, "终端", ShellKind::Zsh);
        session.directory = Some(std::path::PathBuf::from("/home/u/siskin"));
        assert_eq!(session.display_title(), "siskin");
    }

    #[test]
    fn explicit_name_wins_over_the_directory_fallback() {
        let mut session = Session::starting(0, "终端", ShellKind::Zsh);
        session.directory = Some(std::path::PathBuf::from("/home/u/siskin"));
        session.rename(Some("工作".into()));
        assert_eq!(session.display_title(), "工作");
        // 显式名哪怕是「终端」也照显，不再被目录名覆盖。
        session.rename(Some("终端".into()));
        assert_eq!(session.display_title(), "终端");
    }

    #[test]
    fn clearing_the_explicit_name_restores_the_fallback() {
        let mut session = Session::starting(0, "会话 3", ShellKind::Zsh);
        session.directory = Some(std::path::PathBuf::from("/home/u/proj"));
        session.rename(Some("临时".into()));
        assert_eq!(session.display_title(), "临时");
        session.rename(None);
        assert_eq!(session.display_title(), "proj");
        session.rename(Some("".into()));
        assert_eq!(session.display_title(), "proj");
    }
}
