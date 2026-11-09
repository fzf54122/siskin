// @Time    : 2026/11/5 18:10
// @Author  : fzf
// @FileName: shell.rs
// @Software: Siskin

use super::*;

impl SiskinApp {
    pub(super) fn shell_settings(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        group(p)
            .child(group_heading("Shell 与会话", "", p))
            .child(form_row(
                "默认 Shell",
                "仅影响新会话，不会重启已运行的会话。",
                div().h(px(33.)).w_full().flex().children(
                    ShellKind::ALL
                        .into_iter()
                        .enumerate()
                        .map(|(index, shell)| self.shell_choice(index, shell, p, cx)),
                ),
                p,
            ))
            .child(form_row(
                "登录 Shell",
                "来自系统 $SHELL，未设置时回退到 /bin/sh。",
                detail(
                    crate::domain::shell_spec::default_shell()
                        .display()
                        .to_string(),
                    p,
                )
                .font_family(self.font())
                .text_size(px(11.))
                .truncate(),
                p,
            ))
            .child(form_row(
                "新会话目录",
                "新建时继承当前会话的工作目录。",
                detail("继承当前目录", p),
                p,
            ))
    }

    fn shell_choice(
        &self,
        index: usize,
        shell: ShellKind,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let selected = self.settings.shell == shell;
        div()
            .id(("shell", index))
            .debug_selector(move || format!("shell-{index}"))
            .flex_1()
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .border_1()
            .border_color(rgb(if selected { p.accent } else { p.border }))
            .focusable()
            .text_size(px(12.))
            .text_color(rgb(if selected { p.ink } else { p.muted }))
            .cursor_pointer()
            .focus(|style| style.bg(rgb(p.tint)))
            .on_click(cx.listener(move |app, _, _, cx| {
                app.set_shell(shell);
                cx.notify();
            }))
            .on_key_down(cx.listener(move |app, event: &KeyDownEvent, _, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    app.set_shell(shell);
                    cx.notify();
                    cx.stop_propagation();
                }
            }))
            .child(shell.name())
    }
}
