// @Time    : 2026/11/11
// @Author  : fzf
// @FileName: agent_sidebar.rs
// @Software: Siskin

//! Agent 侧栏的应用层操作：开合、节流刷新、点击聚焦。
//!
//! 条目只读会话仓储推导（`domain::agent_sidebar::derive_entry`），
//! 会话新建 / 退出时仓储本身变了，下一次节流刷新自然同步，不另记
//! 增量状态；id 已不在仓储里时聚焦是空操作，不 crash。

use std::time::Instant;

use super::SiskinApp;
use crate::domain::agent_sidebar::{REFRESH_INTERVAL, derive_entry, has_failure};

impl SiskinApp {
    /// 开合。默认收起；打开那下强制刷新一次，免得先看到旧条目。
    pub(crate) fn toggle_agent_sidebar(&mut self) {
        self.agent_sidebar_open = !self.agent_sidebar_open;
        self.agent_sidebar_cache.dirty = true;
        self.request_focus = true;
    }

    /// 侧栏条目缓存：render 每帧都到，但最多 250ms 重算一次
    /// （`REFRESH_INTERVAL`），状态变化不直接触发重算，帧率不受拖累。
    /// 只在终端页 render 里调；设置页没有侧栏，不花这份钱。
    pub(crate) fn refresh_agent_sidebar(&mut self) {
        let cache = &mut self.agent_sidebar_cache;
        if !cache.dirty && cache.refreshed.elapsed() < REFRESH_INTERVAL {
            return;
        }
        let now = Instant::now();
        cache.entries = self
            .sessions
            .iter()
            .map(|(_, session)| {
                derive_entry(
                    session.id,
                    session.display_title(),
                    &session.state,
                    &session.shell_marks,
                    now,
                )
            })
            .collect();
        cache.refreshed = now;
        cache.dirty = false;
    }

    /// 收起时 tab 栏按钮上的红点：缓存条目里任一失败即亮。
    pub(crate) fn agent_sidebar_has_failure(&self) -> bool {
        has_failure(&self.agent_sidebar_cache.entries)
    }

    /// 点侧栏条目：先切到该会话所在的标签，再聚焦它所在的 pane。
    /// id 已不在仓储里（刚退出）时什么都不做，不 crash。
    pub(crate) fn focus_agent_session(&mut self, id: u64) {
        let Some(index) = self.tab_index_containing(id) else {
            return;
        };
        self.select_session(index);
        self.focus_pane(id);
        self.agent_sidebar_cache.dirty = true;
        self.request_focus = true;
    }

    #[cfg(test)]
    pub(crate) fn agent_sidebar_open(&self) -> bool {
        self.agent_sidebar_open
    }
}
