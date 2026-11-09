// @Time    : 2026/11/5 21:30
// @Author  : fzf
// @FileName: prefs_ops.rs
// @Software: Siskin

//! 设置变更 → 防抖保存 → 菜单同步。
//!
//! 拖动取色时每一步都会触发保存，因此改成「标记脏 + 定时落盘」：
//! 500ms 内没有新改动才真正写文件，退出前强制 flush。

use std::time::{Duration, Instant};

use super::SiskinApp;
use crate::io::preferences::{FileStore, Preferences, SessionSnapshot, Store};

/// 改动后的静默期。短于这个间隔的连续改动会合并成一次写入。
const SAVE_DEBOUNCE: Duration = Duration::from_millis(500);

impl SiskinApp {
    pub(crate) fn load_preferences(&mut self, path: std::path::PathBuf) {
        let store = FileStore::new(path);
        self.preferences_path = Some(store.path().to_path_buf());
        match store.load() {
            Ok(preferences) => {
                self.preferences_loaded = true;
                preferences.apply(&mut self.settings, &self.mono_fonts, self.system_dark);
                let shell = self.settings.shell;
                self.sessions.active_mut().kind = shell;
                self.restored = preferences
                    .sessions
                    .iter()
                    .filter(|session| session.is_restorable())
                    .cloned()
                    .collect();
                self.save_error = None;
            }
            // 解析失败时保留原文件，只用默认值启动；覆盖写入会毁掉用户数据。
            Err(error) => {
                self.preferences_loaded = false;
                eprintln!("siskin: 无法读取设置，保留原文件并使用默认值：{error}");
                self.save_error = Some(error.to_string());
            }
        }
    }

    /// 标记配置已变更。真正的写入由 `flush_preferences_if_due` 完成。
    pub(crate) fn mark_dirty(&mut self) {
        self.dirty_since = Some(Instant::now());
    }

    /// 静默期结束后落盘。每帧调用一次，通常是空操作。
    pub(crate) fn flush_preferences_if_due(&mut self) -> bool {
        let Some(since) = self.dirty_since else {
            return false;
        };
        if since.elapsed() < SAVE_DEBOUNCE {
            return false;
        }
        self.dirty_since = None;
        self.save_preferences();
        true
    }

    /// 退出前兜底：无论防抖是否到期都写一次。
    pub(crate) fn flush_preferences(&mut self) {
        if self.preferences_loaded {
            self.dirty_since = None;
            self.save_preferences();
        }
    }

    pub(crate) fn save_preferences(&mut self) {
        let Some(path) = self.preferences_path.clone() else {
            return;
        };
        if !self.preferences_loaded {
            return;
        }
        let store = FileStore::new(path);
        let mut preferences = Preferences::from_settings(&self.settings, &self.mono_fonts);
        preferences.sessions = self.session_snapshots();
        match store.save(&preferences) {
            Ok(()) => self.save_error = None,
            Err(error) => {
                eprintln!("siskin: 无法保存设置：{error}");
                self.save_error = Some(error.to_string());
            }
        }
    }

    pub(crate) fn save_label(&self) -> &'static str {
        if self.save_error.is_some() {
            "保存失败"
        } else if self.dirty_since.is_some() {
            "保存中…"
        } else if self.preferences_path.is_some() {
            "已保存"
        } else {
            "仅本窗口"
        }
    }

    pub(crate) fn preferences_label(&self) -> String {
        self.preferences_path.as_ref().map_or_else(
            || "未配置持久化路径".into(),
            |path| path.display().to_string(),
        )
    }

    pub(crate) fn reveal_preferences(&mut self) {
        if let Some(directory) = self
            .preferences_path
            .as_ref()
            .and_then(|path| path.parent())
            && let Err(error) =
                crate::io::links::open(&crate::io::links::Target::File(directory.to_path_buf()))
        {
            self.status = Some(error);
        }
    }

    /// 只记标题与 cwd，不记输出：恢复历史会让「恢复」变成伪造。
    fn session_snapshots(&self) -> Vec<SessionSnapshot> {
        self.sessions
            .iter()
            .map(|(_, session)| {
                SessionSnapshot::new(
                    session.title.to_string(),
                    session.pty().and_then(|pty| pty.working_directory()),
                )
            })
            .collect()
    }

    /// 设置项改动后统一调用：先同步菜单，再标记待保存。
    pub(crate) fn commit_settings(&mut self) {
        self.sync_menu();
        self.mark_dirty();
    }
}
