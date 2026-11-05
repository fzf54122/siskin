// @Time    : 2026/11/4 20:10
// @Author  : fzf
// @FileName: views.rs
// @Software: Siskin

//! Window chrome and terminal rendering. Only composition and event wiring:
//! text, wrapping and command answers come from `app`, `line` and `shell`.

use crate::{
    app::{CHAR_WIDTH, FONT_SIZE, ROW_HEIGHT, SiskinApp},
    line::{Line, Role},
    theme,
};
use gpui::{
    App, Div, FontWeight, KeyDownEvent, MouseButton, SharedString, Stateful, Window, div,
    prelude::*,
};
use pulse_ui::gpui::{px, rgb};

const SIDEBAR_WIDTH: f32 = 208.;
const RAIL_WIDTH: f32 = 252.;
const TERMINAL_PADDING: f32 = 18.;

impl Render for SiskinApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let width = window.viewport_size().width.as_f32();
        let usable = width
            - SIDEBAR_WIDTH
            - TERMINAL_PADDING * 2.
            - 24.
            - if self.rail { RAIL_WIDTH } else { 0. };
        let columns = ((usable / CHAR_WIDTH).floor().max(40.)) as usize;
        self.session_mut().terminal.set_columns(columns);

        let mut root = div()
            .id("siskin-root")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                let keystroke = &event.keystroke;
                if this.handle_key(
                    &keystroke.key,
                    keystroke.key_char.as_deref(),
                    keystroke.modifiers.control,
                    keystroke.modifiers.platform,
                    keystroke.modifiers.alt,
                    keystroke.modifiers.shift,
                ) {
                    cx.notify();
                    cx.stop_propagation();
                }
            }))
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(theme::CANVAS))
            .font_family(self.mono.clone())
            .text_color(rgb(theme::INK));

        root = root
            .child(self.titlebar(cx))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .min_h_0()
                    .px(px(12.))
                    .pb(px(12.))
                    .gap(px(12.))
                    .child(self.sidebar(cx))
                    .child(self.terminal_card(cx))
                    .when(self.rail, |this| this.child(self.rail(cx))),
            )
            .child(self.statusbar());

        if self.palette.is_some() {
            root = root.child(self.palette_overlay(cx));
        }
        root
    }
}

impl SiskinApp {
    fn titlebar(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .flex_row()
            .items_center()
            .h(px(44.))
            .flex_shrink_0()
            .px(px(18.))
            .gap(px(8.))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(7.))
                    .child(dot(0xe98a7d))
                    .child(dot(0xe8c07a))
                    .child(dot(0x9fc6a3)),
            )
            .child(
                div()
                    .flex_1()
                    .flex()
                    .justify_center()
                    .text_color(rgb(theme::MUTED))
                    .text_xs()
                    .child("Siskin — simulated terminal"),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.))
                    .child(pill(
                        format!("{} 行", self.line_count()),
                        theme::BADGE,
                        theme::MUTED,
                    ))
                    .child(pill("demo".to_string(), theme::TINT, theme::ACCENT))
                    .child(
                        div()
                            .id("toggle-rail")
                            .debug_selector(|| "toggle-rail".into())
                            .px(px(9.))
                            .py(px(4.))
                            .rounded(px(4.))
                            .border_1()
                            .border_color(rgb(theme::BORDER))
                            .bg(rgb(theme::WINDOW))
                            .text_color(rgb(theme::MUTED))
                            .text_xs()
                            .cursor_pointer()
                            .hover(|style| style.bg(rgb(theme::SIDEBAR)))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.rail = !this.rail;
                                cx.notify();
                            }))
                            .child(if self.rail {
                                "隐藏信息栏"
                            } else {
                                "显示信息栏"
                            }),
                    ),
            )
    }

    fn sidebar(&self, cx: &mut Context<Self>) -> Div {
        div()
            .w(px(SIDEBAR_WIDTH))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .h_full()
            .rounded(px(10.))
            .border_1()
            .border_color(rgb(theme::BORDER))
            .bg(rgb(theme::SIDEBAR))
            .py(px(12.))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .px(px(14.))
                    .pb(px(10.))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(theme::FAINT))
                            .child("SESSIONS"),
                    )
                    .child(
                        div()
                            .id("new-session")
                            .debug_selector(|| "new-session".into())
                            .px(px(7.))
                            .py(px(2.))
                            .rounded(px(4.))
                            .text_xs()
                            .text_color(rgb(theme::MUTED))
                            .cursor_pointer()
                            .hover(|style| style.bg(rgb(theme::HAIRLINE)))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.new_session();
                                cx.notify();
                            }))
                            .child("+ 新建"),
                    ),
            )
            .child(div().flex_1().min_h_0().overflow_hidden().children(
                self.sessions.iter().enumerate().map(|(index, session)| {
                    let selected = index == self.active;
                    div()
                        .id(("session", index))
                        .debug_selector(move || format!("session-{index}"))
                        .flex()
                        .flex_col()
                        .gap(px(3.))
                        .mx(px(8.))
                        .mb(px(3.))
                        .px(px(10.))
                        .py(px(8.))
                        .rounded(px(6.))
                        .when(selected, |this| {
                            this.bg(rgb(theme::TINT))
                                .border_1()
                                .border_color(rgb(0xd6e2d1))
                        })
                        .cursor_pointer()
                        .hover(|style| style.bg(rgb(theme::HAIRLINE)))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.select_session(index);
                            cx.notify();
                        }))
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .justify_between()
                                .gap(px(6.))
                                .child(
                                    div()
                                        .text_size(px(12.))
                                        .when(selected, |this| {
                                            this.text_color(rgb(theme::ACCENT))
                                                .font_weight(FontWeight::MEDIUM)
                                        })
                                        .truncate()
                                        .child(session.title.clone()),
                                )
                                .child(
                                    div()
                                        .text_size(px(10.))
                                        .text_color(rgb(theme::FAINT))
                                        .child(session.role),
                                ),
                        )
                        .child(
                            div()
                                .text_size(px(10.))
                                .text_color(rgb(theme::FAINT))
                                .truncate()
                                .child(if session.shell.history().is_empty() {
                                    "空会话".to_string()
                                } else {
                                    session.shell.history().last().cloned().unwrap_or_default()
                                }),
                        )
                }),
            ))
            .child(
                div()
                    .px(px(14.))
                    .pt(px(10.))
                    .mx(px(8.))
                    .border_t_1()
                    .border_color(rgb(theme::BORDER))
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(tiny_hint("Ctrl K", "命令面板"))
                    .child(tiny_hint("Ctrl L", "清屏"))
                    .child(tiny_hint("Ctrl Tab", "切换会话"))
                    .child(tiny_hint("Up / Dn", "历史记录")),
            )
    }

    fn terminal_card(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex_1()
            .min_w(px(320.))
            .h_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(px(10.))
            .border_1()
            .border_color(rgb(theme::BORDER))
            .bg(rgb(theme::TERMINAL))
            .shadow_md()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.focus.focus(window, cx);
                    cx.notify();
                }),
            )
            .child(self.terminal_header())
            .child(self.scrollback(cx))
            .child(self.input_row())
    }

    fn terminal_header(&self) -> Div {
        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .gap(px(10.))
            .px(px(16.))
            .py(px(9.))
            .border_b_1()
            .border_color(rgb(theme::HAIRLINE))
            .bg(rgb(theme::WINDOW))
            .flex_shrink_0()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.))
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(rgb(theme::FAINT))
                            .child("SESSION"),
                    )
                    .child(
                        div()
                            .px(px(8.))
                            .py(px(3.))
                            .rounded(px(4.))
                            .bg(rgb(theme::TINT))
                            .text_color(rgb(theme::ACCENT))
                            .text_size(px(11.))
                            .child(self.session().title.clone()),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(rgb(theme::MUTED))
                            .child(self.session().shell.cwd_label()),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.))
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(rgb(theme::FAINT))
                            .child(format!("{} cols", self.session().terminal.columns())),
                    )
                    .child(dot(theme::SUCCESS))
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(rgb(theme::MUTED))
                            .child("ready"),
                    ),
            )
    }

    fn scrollback(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let columns = self.session().terminal.columns();
        let rows = self.session().terminal.wrapped();
        div()
            .id("scrollback")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .track_scroll(&self.session().scroll)
            .px(px(TERMINAL_PADDING))
            .py(px(12.))
            .cursor_text()
            .on_click(cx.listener(|this, _, window, cx| {
                this.focus.focus(window, cx);
            }))
            .children(rows.into_iter().map(|line| render_line(line, columns)))
    }

    fn input_row(&self) -> Div {
        let input = self.session().terminal.input();
        let empty = input.is_empty();
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(0.))
            .px(px(TERMINAL_PADDING))
            .py(px(10.))
            .border_t_1()
            .border_color(rgb(theme::HAIRLINE))
            .flex_shrink_0()
            .bg(rgb(theme::WINDOW))
            .child(
                div()
                    .flex_shrink_0()
                    .text_size(px(FONT_SIZE))
                    .text_color(rgb(theme::PROMPT))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(format!("{} ", self.session().shell.cwd_label())),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .text_size(px(FONT_SIZE))
                    .text_color(rgb(theme::PROMPT))
                    .child("$ "),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .text_size(px(FONT_SIZE))
                    .text_color(rgb(theme::COMMAND))
                    .when(empty, |this| {
                        this.child(
                            div()
                                .text_color(rgb(theme::FAINT_TEXT))
                                .child("输入 help 开始…"),
                        )
                    })
                    .child(input.to_string())
                    .child(
                        div()
                            .ml(px(1.))
                            .w(px(7.))
                            .h(px(15.))
                            .rounded(px(1.))
                            .bg(rgb(theme::ACCENT))
                            .opacity(0.75),
                    ),
            )
    }

    fn rail(&self, cx: &mut Context<Self>) -> Div {
        let session = self.session();
        let cwd = session.shell.cwd_label();
        let last = session.shell.history().last().cloned();
        div()
            .w(px(RAIL_WIDTH))
            .flex_shrink_0()
            .h_full()
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(
                panel("SESSION")
                    .child(row("标题", session.title.to_string()))
                    .child(row("角色", session.role.to_string()))
                    .child(row("目录", cwd))
                    .child(row("命令", format!("{} 条", self.command_count())))
                    .child(row("行数", self.line_count().to_string())),
            )
            .child(
                panel("最近命令").child(
                    div()
                        .text_size(px(11.))
                        .text_color(rgb(theme::OUTPUT))
                        .child(last.unwrap_or_else(|| "还没有命令".to_string())),
                ),
            )
            .child(
                panel("快捷键")
                    .child(row("Ctrl K", "命令面板"))
                    .child(row("Ctrl L", "清屏"))
                    .child(row("Ctrl Tab", "切换会话"))
                    .child(row("Up / Dn", "历史记录"))
                    .child(row("Tab", "补全")),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(6.))
                    .child(action_button("rail-help", "help", cx, |this| {
                        this.run_help()
                    }))
                    .child(action_button("rail-clear", "清空", cx, |this| {
                        this.clear()
                    })),
            )
            .child(div().flex_1())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .px(px(4.))
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(rgb(theme::FAINT))
                            .child("SCROLLBACK"),
                    )
                    .child(
                        div()
                            .h(px(4.))
                            .w_full()
                            .rounded(px(2.))
                            .bg(rgb(theme::HAIRLINE))
                            .child(
                                div()
                                    .h_full()
                                    .w(gpui::relative(self.scrollback_ratio().max(0.02)))
                                    .rounded(px(2.))
                                    .bg(rgb(theme::ACCENT)),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(rgb(theme::FAINT))
                            .child(format!(
                                "{} / {} 行",
                                self.line_count(),
                                crate::terminal::SCROLLBACK_LIMIT
                            )),
                    ),
            )
    }

    fn statusbar(&self) -> Div {
        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .gap(px(10.))
            .px(px(20.))
            .py(px(6.))
            .flex_shrink_0()
            .border_t_1()
            .border_color(rgb(theme::BORDER))
            .bg(rgb(theme::WINDOW))
            .text_size(px(10.))
            .text_color(rgb(theme::FAINT))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(12.))
                    .child("Siskin 0.1.0")
                    .child("pulse-ui + GPUI")
                    .child("data: local fixture"),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(12.))
                    .child("UTF-8")
                    .child("LF")
                    .child(
                        div()
                            .px(px(6.))
                            .py(px(1.))
                            .rounded(px(3.))
                            .bg(rgb(theme::TINT))
                            .text_color(rgb(theme::ACCENT))
                            .child("no pty"),
                    ),
            )
    }

    fn palette_overlay(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let Some(palette) = self.palette.as_ref() else {
            return div().id("palette-hidden");
        };
        let entries = self.palette_entries();
        let selected = palette.selected;
        let query = palette.query.clone();
        div()
            .id("palette")
            .debug_selector(|| "palette".into())
            .absolute()
            .inset_0()
            .flex()
            .flex_col()
            .items_center()
            .pt(px(96.))
            .bg(gpui::rgba(0x1b1d1f52))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.close_palette();
                    cx.notify();
                }),
            )
            .child(
                div()
                    .w(px(520.))
                    .flex()
                    .flex_col()
                    .rounded(px(10.))
                    .border_1()
                    .border_color(rgb(theme::BORDER))
                    .bg(rgb(theme::WINDOW))
                    .shadow_lg()
                    .overflow_hidden()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.))
                            .px(px(14.))
                            .py(px(11.))
                            .border_b_1()
                            .border_color(rgb(theme::HAIRLINE))
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(rgb(theme::ACCENT))
                                    .child("›"),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .text_size(px(12.))
                                    .text_color(rgb(theme::INK))
                                    .child(if query.is_empty() {
                                        "搜索命令…".to_string()
                                    } else {
                                        query
                                    }),
                            )
                            .child(
                                div()
                                    .text_size(px(10.))
                                    .text_color(rgb(theme::FAINT))
                                    .child("esc"),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .py(px(6.))
                            .max_h(px(320.))
                            .overflow_hidden()
                            .children(entries.into_iter().enumerate().map(|(index, entry)| {
                                let is_selected = index == selected;
                                div()
                                    .id(("palette-entry", index))
                                    .debug_selector(move || format!("palette-{index}"))
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .justify_between()
                                    .gap(px(12.))
                                    .mx(px(6.))
                                    .px(px(10.))
                                    .py(px(8.))
                                    .rounded(px(5.))
                                    .when(is_selected, |this| this.bg(rgb(theme::TINT)))
                                    .cursor_pointer()
                                    .hover(|style| style.bg(rgb(theme::HAIRLINE)))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        let entries = this.palette_entries();
                                        let action = entries
                                            .get(index)
                                            .map(|entry| entry.action)
                                            .unwrap_or(crate::app::PaletteAction::Help);
                                        this.close_palette();
                                        this.run_palette_action(action);
                                        cx.notify();
                                    }))
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap(px(2.))
                                            .child(
                                                div()
                                                    .text_size(px(12.))
                                                    .text_color(rgb(theme::INK))
                                                    .child(entry.label),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(10.))
                                                    .text_color(rgb(theme::FAINT))
                                                    .child(entry.hint),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(10.))
                                            .text_color(rgb(theme::MUTED))
                                            .child(entry.keys),
                                    )
                            })),
                    ),
            )
    }
}

fn render_line(line: Line, _columns: usize) -> Div {
    let mut row = div()
        .flex()
        .flex_row()
        .items_center()
        .h(px(ROW_HEIGHT))
        .flex_shrink_0()
        .text_size(px(FONT_SIZE))
        .whitespace_nowrap();
    for span in line.spans {
        if span.text.is_empty() {
            continue;
        }
        row = row.child(
            div()
                .text_color(rgb(span.role.color()))
                .when(span.role.is_bold(), |this| {
                    this.font_weight(FontWeight::SEMIBOLD)
                })
                .child(SharedString::from(span.text)),
        );
    }
    row
}

fn dot(color: u32) -> Div {
    div().size(px(10.)).rounded_full().bg(rgb(color))
}

fn pill(text: String, background: u32, foreground: u32) -> Div {
    div()
        .px(px(7.))
        .py(px(2.))
        .rounded(px(4.))
        .bg(rgb(background))
        .text_color(rgb(foreground))
        .text_size(px(10.))
        .child(text)
}

fn tiny_hint(keys: &'static str, label: &'static str) -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .gap(px(6.))
        .child(
            div()
                .text_size(px(10.))
                .text_color(rgb(theme::MUTED))
                .child(keys),
        )
        .child(
            div()
                .text_size(px(10.))
                .text_color(rgb(theme::FAINT))
                .child(label),
        )
}

fn panel(title: &'static str) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(6.))
        .p(px(12.))
        .rounded(px(8.))
        .border_1()
        .border_color(rgb(theme::BORDER))
        .bg(rgb(theme::WINDOW))
        .child(
            div()
                .text_size(px(10.))
                .text_color(rgb(theme::FAINT))
                .child(title),
        )
}

fn row(key: impl Into<SharedString>, value: impl Into<SharedString>) -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .gap(px(8.))
        .child(
            div()
                .text_size(px(10.))
                .text_color(rgb(theme::FAINT))
                .child(key.into()),
        )
        .child(
            div()
                .text_size(px(10.))
                .text_color(rgb(theme::INK))
                .truncate()
                .child(value.into()),
        )
}

fn action_button(
    id: &'static str,
    label: &'static str,
    cx: &mut Context<SiskinApp>,
    action: impl Fn(&mut SiskinApp) + 'static,
) -> Stateful<Div> {
    let action = std::rc::Rc::new(action);
    div()
        .id(id)
        .debug_selector(move || id.to_string())
        .flex_1()
        .flex()
        .items_center()
        .justify_center()
        .py(px(5.))
        .rounded(px(4.))
        .border_1()
        .border_color(rgb(theme::BORDER))
        .bg(rgb(theme::WINDOW))
        .text_size(px(10.))
        .text_color(rgb(theme::MUTED))
        .cursor_pointer()
        .hover(|style| style.bg(rgb(theme::HAIRLINE)))
        .on_click(cx.listener(move |this, _, _, cx| {
            action(this);
            cx.notify();
        }))
        .child(label)
}

/// Unused import guard for contexts that only need the type in tests.
#[allow(dead_code)]
fn _assert_app(_: &App) {}

#[allow(dead_code)]
fn role_color(role: Role) -> u32 {
    role.color()
}

#[allow(dead_code)]
fn _unused(_: &mut Window) {}
