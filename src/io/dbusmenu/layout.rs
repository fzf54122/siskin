// @Time    : 2026/11/5 12:38
// @Author  : fzf
// @FileName: layout.rs
// @Software: Siskin

//! 顶层菜单定义。动作由 `MenuItem` 的 id 反查，树结构不携带任何界面类型。

use super::items::{MenuItem, ids};

pub(crate) fn build_layout(theme: &str, shell: &str) -> MenuItem {
    use ids::*;
    MenuItem::submenu(
        ROOT,
        "",
        vec![
            MenuItem::submenu(
                FILE,
                "文件",
                vec![
                    MenuItem::item(NEW_SESSION, "新建会话"),
                    MenuItem::item(CLOSE_SESSION, "关闭当前会话"),
                    MenuItem::separator(101),
                    MenuItem::item(CLEAR, "清空滚动区"),
                    MenuItem::separator(102),
                    MenuItem::item(QUIT, "退出 Siskin"),
                ],
            ),
            MenuItem::submenu(
                EDIT,
                "编辑",
                vec![
                    MenuItem::item(COMMAND_PALETTE, "命令面板"),
                    MenuItem::item(SETTINGS, "设置…"),
                ],
            ),
            MenuItem::submenu(
                VIEW,
                "视图",
                vec![
                    MenuItem::item(THEME_MODE, format!("主题：{theme}（切换模式）")),
                    MenuItem::item(FONT_LARGER, "放大字号"),
                    MenuItem::item(FONT_SMALLER, "缩小字号"),
                    MenuItem::separator(120),
                    MenuItem::item(TOGGLE_INSPECTOR, "显示/隐藏会话信息"),
                    MenuItem::submenu(
                        SHELL,
                        "Shell",
                        vec![
                            MenuItem::checked(SHELL_ZSH, "zsh", "radio", shell == "zsh"),
                            MenuItem::checked(SHELL_BASH, "bash", "radio", shell == "bash"),
                            MenuItem::checked(SHELL_FISH, "fish", "radio", shell == "fish"),
                            MenuItem::checked(SHELL_SH, "sh", "radio", shell == "sh"),
                        ],
                    ),
                ],
            ),
        ],
    )
}
