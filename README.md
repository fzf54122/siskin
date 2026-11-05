<!--
@Time    : 2026/11/4 19:40
@Author  : fzf
@FileName: README.md
@Software: Siskin
-->

# Siskin

用 [`pulse-ui`](https://github.com/fzf54122/resource-pulse) 搭的模拟终端。
窗口、排版、输入焦点和 D-Bus 菜单都是真实的；命令输出和文件树仍是演示数据。
目前不启动 PTY，也没有加载真实 bash/zsh 进程。仅设置会保存到本机配置文件。

<p align="center">
  <img src="assets/siskin-256.png" width="128" alt="Siskin 图标">
</p>

![Siskin 终端](docs/workspace-light.png)

![Siskin 深色终端](docs/workspace-dark.png)

![Siskin 设置页](docs/settings-verified.png)

## 这是什么

一个用来验证 `pulse-ui` 能不能撑起「像终端」的界面实验：
等宽正文、可滚动回看、真实键盘输入、命令面板，以及一整套浅色界面细节。
当前版本仍是终端界面原型，不能替代日常终端。切换 shell 只切换演示会话配置，
不代表真实 shell 进程已启动；真实 PTY、VT 控制序列、全屏 TUI 和输入法尚待实现。

## 运行

```bash
git clone https://github.com/fzf54122/siskin.git
cd siskin
cargo run --locked
```

固定 Rust **1.98.1**。`pulse-ui` 以 Git tag `v0.1.1` 引入，GPUI 与它使用同一个
`zed-industries/zed` 提交 `c427327`，避免同一进程链接两份不兼容的框架。
首次构建需要联网拉取依赖。

需要 X11 / Wayland 图形会话和可用的 Vulkan 驱动。无显示器的终端跑不出窗口，
但测试可以在无显示器环境运行。

Ubuntu/Debian 构建依赖：

```bash
sudo apt-get install pkg-config clang cmake libxkbcommon-dev libxkbcommon-x11-dev \
  libwayland-dev libx11-xcb-dev libxcb1-dev libfontconfig1-dev libssl-dev \
  libzstd-dev libasound2-dev libgit2-dev libvulkan1 mesa-vulkan-drivers dbus-daemon
```

`dbus-daemon` 用于测试时启动隔离总线，不修改桌面正在使用的会话总线。
其他发行版可参考 [`resource-pulse` 的 Linux 章节](https://github.com/fzf54122/resource-pulse#linux)。

## DDE 顶栏菜单

X11 下使用 `com.canonical.dbusmenu`，向
`com.canonical.AppMenu.Registrar` 注册窗口。支持 DDE 的
`dde-top-panel` + `dde-globalmenu-service`，菜单为「文件 / 编辑 / 视图」。

![DDE 顶栏菜单](docs/panel-detail.png)

应用不绘制最小化、最大化和关闭按钮，窗口装饰由桌面负责。
菜单点击通过异步事件唤醒 GPUI，不依赖键盘操作或下一次重绘。
顶栏服务重启后会重新注册；没有顶栏时，仍可使用应用内设置入口和快捷键。
Wayland 全局菜单尚未验证。

菜单窗口属性使用 `_KDE_NET_WM_APPMENU_*` 的 **STRING** 类型。
不写 `_GTK_MENUBAR_OBJECT_PATH`：GTK 的 `org.gtk.Menus` 与 dbusmenu 不是同一种协议。

## 主题与字体

- 默认**跟随系统**，通过 GPUI 的桌面外观通知响应系统深浅色切换。
- 可固定浅色或深色；固定模式不受系统切换影响。
- 自定义主题支持深/浅底以及任意 `#RRGGBB` 强调色、终端底色和文字色。
- 点击颜色值后用 `Ctrl A` 重输，`Enter` 应用，`Esc` 取消；无效颜色不生效。
- 字体从已安装的等宽字体中选择，字号提供 11、12、13、14、16 px。

设置自动保存到 `$XDG_CONFIG_HOME/siskin/settings.json`；
未设置 XDG 路径时使用 `~/.config/siskin/settings.json`。使用临时文件和原子替换保存。
也可以在关闭应用后编辑：

```json
{
  "version": 1,
  "theme_mode": "custom",
  "custom": {
    "dark": true,
    "accent": 0,
    "accent_rgb": "#61aaff",
    "background": "#101827",
    "foreground": "#e8edf2"
  },
  "font_family": "DejaVu Sans Mono",
  "font_size": 13,
  "shell": "bash"
}
```

`theme_mode` 可选 `system`、`light`、`dark`、`custom`。

## 已实现的交互

| 按键 | 行为 |
| --- | --- |
| 直接输入 | 写入当前会话的命令行 |
| `Enter` | 执行命令，输出追加到滚动区 |
| `Tab` | 命令名与路径前缀补全，只在结果唯一时补齐 |
| `Up` / `Down` | 浏览当前会话历史 |
| `PageUp` / `PageDown` | 回看滚动区，输入任意字符后贴回底部 |
| `Esc` | 清空当前输入行 |
| `Ctrl/⌘ K` | 打开命令面板（会话、清屏、重置、快捷键） |
| `Ctrl/⌘ L` | 清屏，保留目录与历史 |
| `Ctrl Tab` / `Ctrl Shift Tab` | 切换会话 |
| `Ctrl/⌘ ,` | 打开设置 |
| `Ctrl/⌘ 1` | 回到终端页 |
| `Ctrl/⌘ Shift I` | 显示 / 收起右侧会话信息 |
| `Ctrl/⌘ Shift T` | 循环切换主题模式 |
| `Ctrl/⌘ +` / `Ctrl/⌘ -` | 放大 / 缩小字号 |

支持的模拟命令：`ls [-l]`、`cd`、`pwd`、`cat`、`echo`、`git status|log`、
`cargo check|test|build`、`whoami`、`date`、`uname`、`siskin`、`help`、`clear`。
所有路径都会被限制在 `fs.rs` 里的只读文件树内。

## 结构

```text
src/
├── main.rs      窗口、应用标识与图标
├── app.rs       会话、状态机、按键分发、命令面板（不依赖渲染）
├── views/
│   ├── sidebar.rs    会话列表与工作区导航
│   ├── workspace.rs  工作区标题栏与页面容器
│   ├── terminal.rs   滚动区、输入行与回到底部
│   ├── inspector.rs  按需展开的会话信息栏
│   ├── palette.rs    命令面板
│   └── settings/     外观、字体、Shell、桌面集成
├── dbusmenu.rs  桌面全局菜单协议与事件桥接
├── preferences.rs 设置加载、校验与原子保存
├── settings.rs  字体、shell 演示配置与主题模式
├── theme/
│   ├── mod.rs       主题模式、自定义配色与对比度
│   └── palettes.rs  内置浅色与深色配色
├── terminal.rs  滚动区与输入行，按列宽软换行
├── shell.rs     命令解析与回答
├── fs.rs        只读的模拟文件树
├── line.rs      带语义角色的文本片段
└── tests.rs     真实焦点、点击与键盘的 UI 测试
```

数据流向是单向的：`views` 只读状态并上抛事件，状态变化在 `app` 里发生，
文本换行在 `line`/`terminal` 完成，命令回答在 `shell` 完成。

## 验证

```bash
cargo fmt --all --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --all-targets --locked
cargo build --locked
neo-runner run -f .agents/loop.yaml --output json
```

测试覆盖补全与历史边界、滚动上限、路径校验、换行样式、设置文件往返、
自定义 RGB 输入、系统主题变化，以及空闲状态下的菜单事件唤醒。
D-Bus 协议测试启动独立总线，校验真实方法调用、子菜单深度和属性。
GUI 测试不能替代实机字体与 GPU 验收。

`scripts/capture_window.py` 可用于 X11 实机验收。它从指定窗口的合成像素图抓取，
不会把遮挡其上的其他应用截进来。该开发工具需要 Python 3、Pillow、
libX11 和 libXcomposite，不是应用运行依赖。

有 X11 桌面时可运行 `python3 scripts/check_desktop.py`，自动启动两次独立实例，
检查菜单属性、空闲时的 D-Bus 动作和深色设置重启恢复。需要 `xdotool`、
`gdbus`、`qdbus`、`xprop` 和 `busctl`；脚本使用临时配置，不改用户设置或重启顶栏服务。

## 设计说明

单窗口、左侧会话栏、连续终端画布，右侧会话信息按需展开；设置留在工作区内，
按「外观 / 字体与排版 / Shell / 桌面集成」分组。浅色与深色使用各自的
中性色层级和同一套布局，默认主题的正文与小字对比度不低于 4.5:1。
图标沿用哑光深色圆球，上面是提示符 `>`、一个星形和一条暖色下划线。

## 许可

Apache-2.0。
