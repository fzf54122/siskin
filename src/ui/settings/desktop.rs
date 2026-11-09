// @Time    : 2026/11/4 23:30
// @Author  : fzf
// @FileName: desktop.rs
// @Software: Siskin

use super::*;

impl SiskinApp {
    pub(super) fn desktop_settings(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .flex_col()
            .gap(px(24.))
            .child(
                group()
                    .child(group_heading(
                        "桌面全局菜单",
                        "将菜单自然地交给桌面顶栏。",
                        p,
                    ))
                    .child(desktop_row("菜单状态", self.menu_state.label(), p))
                    .child(desktop_row("协议", "Canonical dbusmenu · X11", p))
                    .child(desktop_row("窗口控制", "由桌面窗口管理器负责", p))
                    .child(
                        action("settings-dbus-refresh", "刷新菜单", p, cx, |app| {
                            app.sync_menu()
                        })
                        .mt(px(16.)),
                    ),
            )
            .child(
                group()
                    .child(group_heading(
                        "本地偏好设置",
                        "外观和字体自动保存，仅保留在当前设备。",
                        p,
                    ))
                    .child(
                        detail("$XDG_CONFIG_HOME/siskin/settings.json", p).font_family(self.font()),
                    )
                    .child(
                        detail(
                            "没有设置 XDG 路径时，使用 ~/.config/siskin/settings.json。",
                            p,
                        )
                        .mt(px(8.)),
                    ),
            )
            .child(
                detail(
                    "没有全局菜单的桌面上，仍可通过 Ctrl K 打开命令面板，或用 Ctrl , 打开设置。",
                    p,
                )
                .line_height(px(20.)),
            )
    }
}

fn desktop_row(title: &'static str, value: &'static str, p: &Palette) -> Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .py(px(12.))
        .border_b_1()
        .border_color(rgb(p.hairline))
        .child(label(title, p.ink))
        .child(detail(value, p))
}
