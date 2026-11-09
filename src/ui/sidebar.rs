// @Time    : 2026/11/5 18:00
// @Author  : fzf
// @FileName: sidebar.rs
// @Software: Siskin

use super::*;
use crate::domain::{Action, SessionState};

impl SiskinApp {
    /// 会话 tab 条：只占终端上方一条 28px 的横带，替代常驻侧栏。
    /// 每个 tab 显示标题与状态点，关闭按钮只在选中时低透明度可见。
    pub(super) fn session_tabs(&self, p: &Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        div()
            .id("session-tabs")
            .debug_selector(|| "session-tabs".into())
            .flex()
            .items_center()
            .gap(px(2.))
            .h(px(TAB_BAR_HEIGHT))
            .flex_shrink_0()
            .px(px(6.))
            .py(px(2.))
            .bg(rgb(p.window))
            .border_b_1()
            .border_color(rgb(p.hairline))
            .children(
                self.sessions
                    .iter()
                    .enumerate()
                    .map(|(index, _)| self.session_tab(index, p, cx)),
            )
            // 当前工作目录跟在 tab 之后，长路径只保留末级目录；
            // 完整路径仍在系统 titlebar 的副标题位置。
            .child(
                label(self.directory_label(), p.faint)
                    .text_size(px(11.))
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .px(px(8.)),
            )
            // 搜索与会话信息都有快捷键和命令面板条目，不再各占一个图标按钮；
            // tab 条右侧只留新建与设置两个高频入口。
            .child(icon_action(
                "new-session",
                ChromeIcon::Plus,
                "新建会话 · Ctrl+Shift+N",
                false,
                Action::NewSession,
                p,
                cx,
            ))
            .child(icon_action(
                "nav-settings",
                ChromeIcon::Palette,
                "设置 · Ctrl+,",
                false,
                Action::OpenSettings,
                p,
                cx,
            ))
    }

    fn session_tab(&self, index: usize, p: &Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let Some(session) = self.sessions.get(index) else {
            return div().id(("session-missing", index));
        };
        let selected = index == self.active_index();
        div()
            .id(("session", index))
            .debug_selector(move || format!("session-{index}",))
            .relative()
            .h(px(22.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap(px(6.))
            .pl(px(9.))
            .pr(px(7.))
            .rounded(px(4.))
            .cursor_pointer()
            .text_size(px(12.))
            .text_color(rgb(if selected { p.ink } else { p.muted }))
            .bg(rgb(if selected { p.tint } else { 0x00000000 }))
            .hover(|style| style.text_color(rgb(p.ink)).bg(rgb(p.tint)))
            .on_click(cx.listener(move |app, _, _, cx| {
                app.select_session(index);
                app.set_page(Page::Terminal);
                cx.notify();
            }))
            .child(Self::status_dot(session, p))
            .child(
                label(
                    session.display_title(),
                    if selected { p.ink } else { p.muted },
                )
                .flex_1()
                .min_w_0()
                .truncate()
                .when(selected, |view| view.font_weight(FontWeight::MEDIUM)),
            )
            .child(
                self.close_session_button(index, p, cx)
                    .opacity(if selected { 0.7 } else { 0.0 }),
            )
    }

    /// 状态点：运行中绿、启动中琥珀、失败红、已退出灰。
    /// 桌面侧 `SessionState` 已经给出语义，这里只做颜色映射。
    fn status_dot(session: &crate::app::Session, p: &Palette) -> Div {
        let color = match session.state {
            SessionState::Starting => p.warn,
            SessionState::Running => p.success,
            SessionState::Failed(_) => p.error,
            SessionState::Exited(_) => p.faint,
        };
        div()
            .size(px(6.))
            .flex_shrink_0()
            .rounded_full()
            .bg(rgb(color))
    }

    fn close_session_button(
        &self,
        index: usize,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        div()
            .id(("close-session", index))
            .debug_selector(move || format!("close-session-{index}"))
            .size(px(16.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(3.))
            .cursor_pointer()
            .hover(|style| style.bg(rgb(p.border)).opacity(1.))
            .on_click(cx.listener(move |app, _, _, cx| {
                app.close_session(index);
                cx.stop_propagation();
                cx.notify();
            }))
            .child(chrome_icon(ChromeIcon::Close, p.muted).size(px(10.)))
    }
}

pub(super) fn side_rail(p: &Palette) -> Div {
    div()
        .w(px(SETTINGS_RAIL_WIDTH))
        .flex_shrink_0()
        .h_full()
        .flex()
        .flex_col()
        .bg(rgb(p.sidebar))
        .border_r_1()
        .border_color(rgb(p.border))
}

pub(super) fn side_action(
    id: &'static str,
    title: &'static str,
    keys: &'static str,
    icon: ChromeIcon,
    p: &Palette,
    cx: &mut Context<SiskinApp>,
    callback: impl Fn(&mut SiskinApp) + 'static,
) -> Stateful<Div> {
    chrome_action(id, "", p, cx, callback)
        .w_full()
        .h(px(35.))
        .gap(px(10.))
        .justify_start()
        .child(chrome_icon(icon, p.muted))
        .child(label(title, p.ink).text_size(px(12.)))
        .child(div().flex_1())
        .child(detail(keys, p).text_size(px(10.)))
}
