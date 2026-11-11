// @Time    : 2026/11/11
// @Author  : fzf
// @FileName: agent_sidebar.rs
// @Software: Siskin

//! Agent 侧栏渲染：只呈现已有会话的状态。条目读 `agent_sidebar_cache.entries`
//! 缓存（250ms 节流重算），点击聚焦、失败给徽章；与命令面板、ghost
//! text、滚动条互不冲突——它是主工作区右侧的一块独立面板，不盖终端。

use super::*;
use crate::domain::agent_sidebar::{AGENT_SIDEBAR_WIDTH, AgentEntry, EntryStatus};

impl SiskinApp {
    pub(super) fn agent_sidebar(&self, p: &Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        div()
            .id("agent-sidebar")
            .debug_selector(|| "agent-sidebar".into())
            .w(px(AGENT_SIDEBAR_WIDTH))
            .flex_shrink_0()
            .h_full()
            .flex()
            .flex_col()
            .relative()
            .bg(rgba(self.settings.glass_surface(p.sidebar, p.dark)))
            .border_l_1()
            .border_color(rgba(glass_edge(p)))
            .child(glass_top_highlight())
            .child(self.agent_sidebar_header(p, cx))
            .child(
                div()
                    .id("agent-sidebar-body")
                    .debug_selector(|| "agent-sidebar-body".into())
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .when(self.agent_sidebar_cache.entries.is_empty(), |view| {
                        view.child(detail("暂无会话", p).text_size(px(12.)).p(px(12.)))
                    })
                    .children(
                        self.agent_sidebar_cache
                            .entries
                            .iter()
                            .map(|entry| self.agent_entry(entry, p, cx)),
                    ),
            )
    }

    fn agent_sidebar_header(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .items_center()
            .justify_between()
            .h(px(40.))
            .px(px(12.))
            .border_b_1()
            .border_color(rgb(p.border))
            .child(
                label("Agent", p.ink)
                    .text_size(px(13.))
                    .font_weight(FontWeight::MEDIUM),
            )
            .child(action("close-agent-sidebar", "×", p, cx, |app| {
                app.agent_sidebar_open = false;
            }))
    }

    /// 一条会话：状态点 + 标题 + 失败徽章，第二行是当前 / 最近命令，
    /// 第三行是状态文案（耗时 / 退出码）。点整条聚焦到对应会话。
    fn agent_entry(
        &self,
        entry: &AgentEntry,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let id = entry.session_id;
        let active = id == self.active_pane;
        let dot = Self::agent_status_color(entry.status, p);
        let failed = entry.failed;
        let command = entry.command.clone();
        div()
            .id(("agent-entry", id))
            .debug_selector(move || format!("agent-entry-{id}"))
            .px(px(12.))
            .py(px(8.))
            .flex()
            .flex_col()
            .gap(px(2.))
            .cursor_pointer()
            .bg(rgb(if active { p.tint } else { 0x00000000 }))
            .hover(|style| style.bg(rgb(p.tint)))
            .border_b_1()
            .border_color(rgb(p.hairline))
            .on_click(cx.listener(move |app, _, _, cx| {
                app.focus_agent_session(id);
                cx.notify();
            }))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(
                        div()
                            .size(px(6.))
                            .flex_shrink_0()
                            .rounded_full()
                            .bg(rgb(dot)),
                    )
                    .child(
                        label(entry.title.clone(), if active { p.ink } else { p.muted })
                            .text_size(px(12.))
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .when(active, |view| view.font_weight(FontWeight::MEDIUM)),
                    )
                    .when(failed, |view| {
                        view.child(
                            div()
                                .id(("agent-failed-badge", id))
                                .debug_selector(move || format!("agent-failed-badge-{id}"))
                                .px(px(5.))
                                .py(px(1.))
                                .rounded(px(3.))
                                .bg(rgb(p.error))
                                .child(label("失败", 0xFFFFFF).text_size(px(10.))),
                        )
                    }),
            )
            .when(command.is_some(), |view| {
                view.child(
                    label(command.unwrap_or_default(), p.ink)
                        .font_family(self.mono.clone())
                        .text_size(px(12.))
                        .truncate(),
                )
            })
            .child(detail(entry.detail.clone(), p).text_size(px(11.)))
    }

    /// 状态点颜色：tab 栏 `status_dot` 的同款映射，运行中与完成都是绿，
    /// 失败红、启动中琥珀、空闲与已退出灰，文案区分运行中与完成。
    fn agent_status_color(status: EntryStatus, p: &Palette) -> u32 {
        match status {
            EntryStatus::Starting => p.warn,
            EntryStatus::Running | EntryStatus::Succeeded => p.success,
            EntryStatus::Failed | EntryStatus::SpawnFailed => p.error,
            EntryStatus::Idle | EntryStatus::Exited => p.faint,
        }
    }
}
