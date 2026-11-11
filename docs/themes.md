<!--
@Time    : 2026/11/11
@Author  : fzf
@FileName: themes.md
@Software: Siskin
-->

# 配色

六套内置预设：Paper、Graphite、Moss、Dusk（自研四套）与 Catppuccin
Mocha、Tokyo Night（上游色板移植）。命令面板（Ctrl+K）里搜「主题」
可直接点选任意一套；Ctrl+Shift+T 仍是按模式循环（跟随系统 / 浅色 /
深色 / 自定义），点选过的预设在换模式之前一直生效，并随设置落盘。

## 对比度标尺

所有预设的主文字、次要文字、弱化文字、强调色（在选区底上）、终端
输出文字，与各自底色的 WCAG 对比度都不低于 **4.5:1**；光标色就是
强调色，与终端底色的对比度同样不低于 4.5:1。断言在
`src/theme/tests.rs`（`default_themes_keep_small_text_readable`、
`cursor_accent_stays_visible_on_the_terminal_background`），逐套跑，
不放宽阈值。

## 四套自研配色的修整（2026-11-11）

修整只改色值，不改配色系统结构（`Palette` 字段、对比度逻辑、
`ThemeId` 枚举形状都不动）；每处改动都有实测对比度数值做依据。

- **Paper**：强调色原来与正文同色（`#3b3d42`），光标、活动标签
  下划线、焦点边框与正文分不开；改成深 slate 蓝 `#2f5b8f`
  （白底 7.0:1，选区底 5.6:1）。选区底 `#e9edf3` 太接近纯白，
  加深到 `#dde7f0`；`accent_soft` 跟着改成带蓝的 `#c9d6e4`。
  警告色 `#8f703f` 只有 4.6:1，余量太小，压深到 `#7d5f31`（5.9:1）。
- **Graphite**：六项断言全部通过，无刺眼、无发灰，未改动。
- **Moss**：弱化文字 `#8d9a86` 只有 2.8:1、终端弱化文字
  `#9aa691` 只有 2.5:1、系统消息色 `#76836f` 只有 3.9:1，全部不达
  标。`muted` 从 `#66735f` 压深到 `#5b6854`（5.5:1），给弱化色
  留出梯度；`faint`/`faint_text` 取旧 `muted` 的值 `#66735f`
  （窗口 4.7:1、终端 4.9:1）；`system` 与 `muted` 对齐。
- **Dusk**：弱化文字 `#717892` 只有 3.6:1、终端弱化文字 `#646b82`
  只有 3.3:1。`faint`/`faint_text` 改成 `#8b92a8`（窗口 5.1:1、
  终端 5.7:1），与原 `system` 同值，仍弱于 `muted`（6.5:1）。

## Catppuccin Mocha

色值来源：Catppuccin 官方色板（`catppuccin/catppuccin` 的 palette
数据，Mocha 风味；另见同项目各语言移植版交叉核对）。

| 角色 | token | 色值 |
| --- | --- | --- |
| 窗口底 | base | `#1e1e2e` |
| 终端底 | mantle | `#181825` |
| 画布底 | crust | `#11111b` |
| 正文 | text | `#cdd6f4` |
| 次要文字 | subtext1 | `#bac2de` |
| 弱化文字 | subtext0 | `#a6adc8` |
| 强调色 / 光标 | mauve | `#cba6f7` |
| 选区底 | surface0 | `#313244` |
| 成功 | green | `#a6e3a1` |
| 提示 | pink | `#f5c2e7` |
| 路径 | blue | `#89b4fa` |
| 警告 | yellow | `#f9e2af` |
| 错误 | red | `#f38ba8` |
| 链接 | sky | `#89dceb` |

终端底用 mantle 而不用 base：与应用内其余深色主题一致（终端底比
窗口底深一档），且测试要求同一主题的窗口底与终端底不同。

## Tokyo Night（Night 变体）

色值来源：Tokyo Night 官方仓库（`tokyonight.nvim` 的 Night 变体色板，
与 `tokyo-night-vscode-theme` 的终端色表交叉核对）。

| 角色 | token | 色值 |
| --- | --- | --- |
| 窗口底 | bg | `#1a1b26` |
| 终端底 | bg_dark | `#16161e` |
| 正文 | fg | `#c0caf5` |
| 次要文字 | fg_dark | `#a9b1d6` |
| 强调色 / 光标 | blue | `#7aa2f7` |
| 选区底 | bg_visual | `#283457` |
| 边框 | bg_highlight | `#292e42` |
| 成功 | green | `#9ece6a` |
| 提示 | magenta | `#bb9af7` |
| 路径 | blue | `#7aa2f7` |
| 警告 | yellow | `#e0af68` |
| 错误 | red | `#f7768e` |
| 链接 | cyan | `#7dcfff` |

一处派生值：官方弱化色 dark5 `#737aa2` 在窗口底上只有 4.1:1，
未达 4.5，往 fg 方向提亮一档为 `#7f8bb3`（窗口 5.1:1、终端
5.4:1），仍弱于次要文字（8.1:1）。其余色值均为官方原值。

## 备注

- ANSI 16 色表仍按深浅两档共用（`src/io/ansi.rs`），不随预设变；
  只有前景、背景、光标三色取自当前预设。
- 自定义主题（自选底色与强调色）仍走 `ensure_contrast` 校正，
  不会被不可读的组合设废；与预设无关。
