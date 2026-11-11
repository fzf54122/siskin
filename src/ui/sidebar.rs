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
                self.tab_indices()
                    .into_iter()
                    .map(|index| self.session_tab(index, p, cx)),
            )
            // 松开时落在 tab 间空隙或目录 label 上：tab 自己的
            // `on_mouse_up` 收不到，这里收尾，`end_tab_drag` 是幂等的。
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|app, _, _, cx| {
                    app.end_tab_drag();
                    cx.notify();
                }),
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
            // tab 条右侧只留新建、Agent 侧栏与设置三个高频入口。
            .child(icon_action(
                "new-session",
                ChromeIcon::Plus,
                "新建会话 · Ctrl+Shift+N",
                false,
                Action::NewSession,
                p,
                cx,
            ))
            .child(self.agent_sidebar_button(p, cx))
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

    /// Agent 侧栏开关：打开时按钮高亮；有失败命令时右上角给 6px 红点，
    /// 侧栏收起也能看见。
    fn agent_sidebar_button(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        div()
            .relative()
            .flex_shrink_0()
            .child(icon_action(
                "toggle-agent-sidebar",
                ChromeIcon::List,
                "Agent 侧栏 · Ctrl+Shift+A",
                self.agent_sidebar_open,
                Action::ToggleAgentSidebar,
                p,
                cx,
            ))
            .when(self.agent_sidebar_has_failure(), |view| {
                view.child(
                    div()
                        .absolute()
                        .top(px(3.))
                        .right(px(3.))
                        .size(px(6.))
                        .rounded_full()
                        .bg(rgb(p.error)),
                )
            })
    }

    fn session_tab(&self, index: usize, p: &Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let Some(session) = self.sessions.get(index) else {
            return div().id(("session-missing", index));
        };
        let selected = index == self.active_index();
        let editing = self
            .tab_rename
            .as_ref()
            .is_some_and(|rename| rename.index == index);
        let dragging = self.tab_dragging(index);
        let tab = div()
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
            // 被拖的标签半透明；改名中的标签底边一条强调色线。
            .when(dragging, |view| view.opacity(0.5))
            .when(editing, |view| {
                view.border_b_1().border_color(rgb(p.accent))
            });
        let tab = Self::tab_drag_handlers(index, cx, tab);
        let tab = Self::tab_click_handler(index, cx, tab);
        tab.child(Self::status_dot(session, p))
            .child(if editing {
                self.rename_editor(p)
            } else {
                label(
                    session.display_title(),
                    if selected { p.ink } else { p.muted },
                )
                .flex_1()
                .min_w_0()
                .truncate()
                .when(selected, |view| view.font_weight(FontWeight::MEDIUM))
            })
            .when(!editing, |view| {
                view.child(
                    self.close_session_button(index, p, cx)
                        .opacity(if selected { 0.7 } else { 0.0 }),
                )
            })
    }

    /// 拖拽的三个鼠标回调：按下记起始标签、移过别的标签时当场换位、
    /// 松开收尾。只在按住左键移动时换位，纯悬停不动。
    fn tab_drag_handlers(
        index: usize,
        cx: &mut Context<Self>,
        tab: Stateful<Div>,
    ) -> Stateful<Div> {
        tab.on_mouse_down(
            MouseButton::Left,
            cx.listener(move |app, _, _, cx| {
                app.start_tab_drag(index);
                cx.notify();
            }),
        )
        .on_mouse_move(
            cx.listener(move |app, event: &gpui::MouseMoveEvent, _, cx| {
                if event.pressed_button == Some(MouseButton::Left) {
                    app.hover_tab_drag(index);
                    cx.notify();
                }
            }),
        )
        .on_mouse_up(
            MouseButton::Left,
            cx.listener(move |app, _, _, cx| {
                app.end_tab_drag();
                cx.notify();
            }),
        )
    }

    /// 单击切标签，双击进重命名；拖拽换过位后松开合成的 click 要吞掉。
    fn tab_click_handler(
        index: usize,
        cx: &mut Context<Self>,
        tab: Stateful<Div>,
    ) -> Stateful<Div> {
        tab.on_click(cx.listener(move |app, event: &gpui::ClickEvent, _, cx| {
            if app.tab_click_suppressed {
                app.tab_click_suppressed = false;
                return;
            }
            // 点正在改的标签本身不提交（点别处才失焦提交），继续改。
            if app
                .tab_rename
                .as_ref()
                .is_some_and(|rename| rename.index == index)
            {
                return;
            }
            if event.click_count() == 2 {
                app.begin_tab_rename(index);
            } else {
                app.select_session(index);
                app.set_page(Page::Terminal);
            }
            cx.notify();
        }))
    }

    /// 重命名编辑框：光标处一条 1px 强调色竖线，选中段上 tint 底色。
    /// 文本分三段（选区前、选区内、选区后），方向键、Home/End、
    /// Shift 选区由 `rename_key` 改 `TextEdit`，这里只按它画。
    /// 不支持点按定位光标（点击本标签是保持编辑），见 plan.md。
    fn rename_editor(&self, p: &Palette) -> Div {
        let Some(rename) = self.tab_rename.as_ref() else {
            return div();
        };
        let edit = &rename.edit;
        let selection = edit.selection();
        let text = edit.text.as_str();
        let before = &text[..selection.start];
        let selected = &text[selection.clone()];
        let after = &text[selection.end..];
        let cursor_at_start = edit.cursor <= selection.start;
        div()
            .flex()
            .items_center()
            .flex_1()
            .min_w_0()
            .overflow_hidden()
            .child(label(before, p.ink).flex_shrink_0().text_size(px(12.)))
            .when(cursor_at_start, |view| {
                view.child(div().w(px(1.)).h(px(14.)).flex_shrink_0().bg(rgb(p.accent)))
            })
            .when(!selected.is_empty(), |view| {
                view.child(
                    label(selected, p.ink)
                        .flex_shrink_0()
                        .text_size(px(12.))
                        .bg(rgb(p.tint))
                        .px(px(1.)),
                )
            })
            .when(!cursor_at_start, |view| {
                view.child(div().w(px(1.)).h(px(14.)).flex_shrink_0().bg(rgb(p.accent)))
            })
            .child(label(after, p.ink).flex_shrink_0().text_size(px(12.)))
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

pub(super) fn side_rail(p: &Palette, rail_bg: u32) -> Div {
    div()
        .w(px(SETTINGS_RAIL_WIDTH))
        .flex_shrink_0()
        .h_full()
        .flex()
        .flex_col()
        .bg(rgba(rail_bg))
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
