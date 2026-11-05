// @Time    : 2026/11/4 23:30
// @Author  : fzf
// @FileName: shell.rs
// @Software: Siskin

use super::*;

impl SiskinApp {
    pub(super) fn shell_settings(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        div().flex().flex_col().gap(px(24.))
            .child(
                group(p)
                    .child(group_heading("默认 Shell", "新建会话时使用的预览配置。", p))
                    .children(ShellKind::ALL.into_iter().enumerate().map(|(index, shell)| {
                        self.shell_choice(index, shell, p, cx)
                    })),
            )
            .child(
                div().p(px(18.)).rounded(px(8.)).bg(rgb(p.tint))
                    .child(label("当前是界面预览版本", p.ink).font_weight(FontWeight::MEDIUM))
                    .child(detail(
                        "真实 PTY 与 Shell 进程尚未接入。这里的选择不会启动程序，也不会修改系统默认 Shell。",
                        p,
                    ).mt(px(8.)).line_height(px(20.))),
            )
    }

    fn shell_choice(
        &self,
        index: usize,
        shell: ShellKind,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let selected = self.settings.shell == shell;
        let description = match shell {
            ShellKind::Zsh => "灵活的交互与扩展生态",
            ShellKind::Bash => "熟悉、通用的命令行环境",
            ShellKind::Fish => "友好的交互式 Shell",
            ShellKind::Sh => "简洁的 POSIX Shell",
        };
        div()
            .id(("shell", index))
            .debug_selector(move || format!("shell-{index}"))
            .flex()
            .items_center()
            .gap(px(14.))
            .px(px(14.))
            .py(px(14.))
            .rounded(px(7.))
            .bg(rgb(if selected { p.tint } else { p.window }))
            .cursor_pointer()
            .hover(|style| style.bg(rgb(p.sidebar)))
            .on_click(cx.listener(move |app, _, _, cx| {
                app.set_shell(shell);
                cx.notify();
            }))
            .child(
                div()
                    .size(px(16.))
                    .rounded_full()
                    .border_2()
                    .border_color(rgb(if selected { p.accent } else { p.muted }))
                    .bg(rgb(if selected { p.accent } else { p.window })),
            )
            .child(
                div()
                    .flex_1()
                    .child(label(shell.name(), p.ink).font_weight(FontWeight::MEDIUM))
                    .child(detail(description, p).mt(px(5.))),
            )
            .when(selected, |view| view.child(detail("默认", p)))
    }
}
