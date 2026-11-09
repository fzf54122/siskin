<!--
@Time    : 2026/11/5 10:55
@Author  : fzf
@FileName: plan.md
@Software: Siskin
-->

# Siskin 从演示走向可用终端

## 1. 目标

把 Siskin 从「界面演示」变成每天能开的 Linux 终端：真实 PTY、真实 VT 解析、真实输入法、真实复制粘贴、可安装可固定到 Plank。

### 完成的样子

- 打开就是自己的登录 shell，能跑 `vim`、`htop`、`ssh`、`tmux`、`cargo watch`，不乱码不错位。
- 中文输入法能组词、能退格、候选框跟光标走。
- 鼠标选中复制、`Ctrl+Shift+V` 粘贴、`Shift+PageUp` 回看，与常见终端一致。
- 关掉窗口再开，主题、字体、shell、滚动行数都还在。
- `apt install ./siskin_x.y.z_amd64.deb` 之后，开始菜单、Plank、DDE 顶栏菜单都能找到它。

### 验收的硬标准

用一整天做真实工作，不切回原终端。任何一次要切回去，那一条就记成 bug。

## 2. 现状盘点

仓库已经推进过一轮，下面的表格按当前工作区的真实结构写（不要按旧版 `views.rs`/`theme.rs` 单文件去理解）。

已完成、可以保留的部分：

| 部分 | 现状 | 后续 |
| --- | --- | --- |
| 窗口与布局 | `views/` 已拆成 `sidebar / workspace / terminal / inspector / palette / preview / widgets / settings/` | 保留，只换终端卡片内部 |
| 主题系统 | `theme/` 已拆成 `mod / colors / palettes / fonts / tests`，支持预设 + 自定义取色 | 补 ANSI 16/256 色表 |
| 设置页 | `views/settings/` 分 `appearance / colors / fonts / shell / desktop` 五个分区 | 接真实 shell 路径与持久化 |
| 配置持久化 | `preferences.rs` 已实现：版本号、字体按名字存、原子替换、损坏不覆盖 | 抽 `Store` trait + 迁移链 |
| 全局菜单 | `dbusmenu.rs` 696 行，`com.canonical.dbusmenu` + Registrar 已跑通 | 拆成 `dbusmenu/` 目录 |
| 命令面板 | `views/palette.rs` + `app.rs` 的 `CommandPalette` | 换真实动作 |
| `line.rs` | `Role/Span` 语义化文本片段 | 软换行逻辑作废，角色保留 |
| GUI 测试 | `tests.rs` 316 行，`TestAppContext` 真点击真按键 | 迁到 `tests/` 目录 |

必须换掉的部分：

| 现有 | 去向 | 原因 |
| --- | --- | --- |
| `shell.rs` 模拟命令引擎（560 行） | 删除 | 真实 shell 由 PTY 提供 |
| `fs.rs` 只读文件树（191 行） | 删除 | 同上 |
| `terminal.rs` 自维护文本与软换行（202 行） | 重写 | 终端要按网格与 ANSI 状态渲染，不是按行软换行 |
| `ShellKind::prompt_marker/startup` | 删除 | 提示符由 shell 自己输出，假提示符会盖住真提示符 |
| `views/terminal.rs` 的 `welcome()` 文案「当前为界面预览」 | 删除 | 不再是预览 |
| `assets/siskin.svg` 与两个 PNG | 替换 | 已有新图标设计，当前还是旧的深色圆球 |

结构债（见第 6 章，P0 处理）：

| 文件 | 行数 | 问题 |
| --- | --- | --- |
| `app.rs` | 1075 | 一个文件装了会话、面板、按键链、设置操作、菜单桥接、测试 |
| `dbusmenu.rs` | 696 | 菜单树构造、D-Bus 服务、布局序列化、协议测试混在一起 |
| `shell.rs` | 560 | 待删除，删除前不再加功能 |
| `tests.rs` | 316 | 待迁 `tests/` 目录 |

`views/` 和 `theme/` 已经是拆分后的好状态（最大文件 `views/settings/colors.rs` 172 行），后续按同一粒度写，别退回去。

## 3. 非目标

先明确不做，避免范围失控：

- 不做 SSH 客户端、文件传输、端口转发面板。
- 不做分屏（split pane）与标签拖拽重排，P6 之后再看。
- 不做 Windows / macOS 安装包，Linux 之外只保证能编译。
- 不做 GPU 字形图集（glyph atlas）自研；先用 GPUI 的文本整形，性能不够再优化。
- 不追求 100% 兼容 terminfo 之外的小众序列（sixel、kitty graphics protocol）。

## 4. 技术选型

### 4.1 VT 解析与 PTY：`alacritty_terminal`

自己写 VTE 解析器是最大的时间黑洞，而且 correctness 极难保证。用 Alacritty 的 `alacritty_terminal` 作为后端：

```
alacritty_terminal = "=0.26.0"
```

- 提供 `Term<T>`（网格 + 滚动区 + 选区 + 链接 + 鼠标协议 + kitty keyboard 状态）、`vte::Parser`、`tty::new`（openpty + fork + setsid + TIOCSWINSZ）。
- 版本 0.26.0 要求 Rust 1.85，与仓库固定的 1.98.1 兼容；许可证 Apache-2.0。
- 依赖 `vte 0.15`、`rustix-openpty`，都是成熟实现。`vte` 由上游重新导出，不单独声明版本。

隔离策略：只在 `src/vt.rs` 里出现 `alacritty_terminal` 的类型，其余模块只看见自己的 `Screen`/`Cell`/`Event` 抽象。这样以后换 `wezterm` 的 `termwiz` 成本可控。

需要注意的耦合：Zed 的 `crates/terminal` 内部锁定 `alacritty_terminal 0.26.1-dev`（Git），与本项目的 0.26.0（crates.io）不是同一个 package 来源，Cargo 会当成两个 crate。Siskin 不依赖 Zed 的 terminal crate，所以只会编译一份，但要避免将来误引用。

### 4.2 渲染：自定义 GPUI `Element`

GPUI 没有终端网格组件，需要实现 `gpui::Element`：

- `request_layout`：占满可用区域，按 `cell_width × cols`、`row_height × rows` 计算，取整到 cell 边界，余量分给 padding。
- `prepaint`：只为可视区 + 上下各一行准备整形结果，越界行不做工作。
- `paint`：逐行铺背景矩形（按 `Cell.bg` 合并相邻同色）、再画文本、最后画光标与选区。
- 文本整形：每行把 cell 聚成 run（同 fg/bg/bold/italic/underline 合并），一次 `shape_line` 出整行，交给 `ShapedLine::paint`。比逐 cell 整形快一个量级，也天然支持连字。
- 缓存：`shape_line_by_hash` 以「行内容 + 样式 + 字体 + 字号」为键，滚动回看不重复整形。
- 裁剪：容器 `overflow_hidden`，绘制时只写可视行，不靠 GPU 裁剪大缓冲区。

双宽字符（CJK、emoji）看 `Flags::WIDE_CHAR` / `WIDE_CHAR_SPACER`，不从字符宽度猜；组合字符在 `Cell::zerowidth()`，跟在前一个 cell 上绘制。cell 宽度来源是 `TextSystem::advance`。

### 4.3 输入法

实现 `gpui::EntityInputHandler`，通过 `window.handle_input(focus_handle, handler, cx)` 注册：

- `replace_and_mark_text_in_range` 接收预编辑文本，直接渲染成带下划线的候选串，提交才写 PTY。
- `selected_text_range` 返回光标所在的 UTF-16 位置；`InputHandler::selected_bounds` 给出候选框锚点。
- 光标移动或窗口滚动后调用 `window.invalidate_character_coordinates()`，GPUI 会在下一帧把 `selected_bounds` 同步给平台。

X11 走 XIM，Wayland 走 `text-input-v3`（GPUI 的 `gpui_linux` 已实现 `set_ime_cursor_rectangle`）。Wayland 下输入法可用性必须在 P3 单独验收，这是最容易翻车的点。

### 4.4 键位映射

不自己拼 escape 序列，用 `alacritty_terminal` 的语义：把 GPUI 的 `Keystroke` 归一成 `(key, mods, logical_key)`，映射到终端要写入的字节。

规则：

- 普通按键写入字符本身；`Ctrl+字母` 写控制码；`Alt+键` 写 `ESC` 前缀。
- 方向键、F1–F12、Home/End/Insert/Delete 依赖 `TermMode`（DECCKM 等）在 CSI 与 SS3 之间选。
- 应用开启 kitty keyboard protocol（`CSI > 1 u`）时按协议编码。
- 应用开启鼠标上报（`TermMode::MOUSE_*`）时，鼠标事件转 SGR 序列发给程序；否则本地选区。
- 优先级：应用快捷键 > 终端快捷键 > 程序输入。终端快捷键统一走 `Ctrl+Shift+*`，避开 shell 常用组合。

### 4.5 配置持久化

`preferences.rs` 已经落地了主体，这一节写清现状与剩余工作，不要重复实现：

已实现：

- 路径：`$XDG_CONFIG_HOME/siskin/settings.json`，未设置时 `~/.config/siskin/settings.json`；`XDG_CONFIG_HOME` 非绝对路径时被忽略。
- 格式：`serde` + `serde_json`，带 `version: u32` 与 `deny_unknown_fields`，读取上限 64 KiB。
- 字体按**名字**存而不是列表下标，换机器或换字体集不会串位。
- 写入：`tempfile::NamedTempFile` + `sync_all` + `persist` 原子替换。
- 损坏：解析失败时只告警并保留原文件，用默认值启动，绝不覆盖用户配置。

剩余工作（P0/P5）：

- 抽出 `Store` trait，文件实现与内存实现分开，测试不再 `tempfile::tempdir()` 落盘。
- 版本迁移改成责任链：`v1 → v2` 逐级处理，遇到跳跃版本直接报错。
- 现在的保存是「每次变更立刻同步写」，设置页拖动取色时会连续写盘。改成 500ms 防抖 + `on_app_quit` 兜底 flush。
- 写失败时 UI 要有反馈（现在只有 `eprintln`），至少在设置页显示一行「未能保存」。

### 4.6 单实例与命令行

- D-Bus 名字 `io.github.fzf.Siskin.Instance`，复用现有 `zbus` 依赖。
- 第二次启动：转发 `--working-directory`、`--new-window` 后退出，不创建第二个 GPU 上下文。
- `siskin -e <command> [args...]` 直接执行命令，退出即关窗（终端模拟器惯例，脚本与 IDE 集成要用）。

## 5. 目标架构

```text
src/
├── main.rs           入口：命令行解析、单实例、窗口、菜单注册
├── app/              应用状态，mod.rs 只放结构与组装
│   ├── mod.rs        SiskinApp、构造、apply(Change)
│   ├── session.rs    会话仓储 Sessions（增删查、active 切换）
│   ├── keys.rs       按键责任链（KeyStage）
│   ├── actions.rs    Action 枚举 + dispatch（命令模式）
│   ├── palette.rs    命令面板状态与过滤
│   └── prefs_ops.rs  设置变更 → 保存 → 菜单同步
├── domain/           纯数据与规则，不 use gpui、不 use zbus
│   ├── session.rs    SessionState 状态机、SessionSpec
│   ├── shell_spec.rs ShellSpec：Default / Program / WithArgs
│   └── metrics.rs    RenderMetrics（字体度量、行列数换算）
├── io/               系统边界，不 use gpui
│   ├── pty.rs        PTY 派生、读写线程、resize、子进程回收
│   ├── vt.rs         alacritty_terminal 封装：Term、Parser、事件、选区
│   ├── ansi.rs       ANSI/256/truecolor → Palette 解析
│   ├── links.rs      链接识别、cwd 相对路径、协议白名单
│   ├── search.rs     正则搜索与高亮
│   ├── preferences/  版本化配置
│   │   ├── mod.rs    Preferences 数据与 from/apply
│   │   ├── store.rs  Store trait：文件 / 内存
│   │   └── migrate.rs 版本迁移责任链
│   └── dbusmenu/     全局菜单
│       ├── mod.rs    install 与生命周期
│       ├── items.rs  MenuItem 树与递归查找
│       ├── service.rs com.canonical.dbusmenu 接口实现
│       └── layout.rs 布局序列化与顶层菜单定义
├── ui/               GPUI 视图，不直接碰 std::process
│   ├── mod.rs        Render 实现与根布局
│   ├── grid.rs       自定义 Element：布局、整形缓存、光标、选区
│   ├── widgets.rs    可复用积木
│   ├── sidebar.rs / workspace.rs / inspector.rs / palette.rs / preview.rs
│   ├── terminal/     card.rs / scrollback.rs / input.rs
│   └── settings/     appearance / colors / fonts / shell / desktop
├── line.rs           语义化文本片段（欢迎语、错误提示）
└── tests/            分层测试：keys / views / menu / session / pty
```

数据流单向：

```
键盘/输入法 ─→ keys.rs ─→ PTY 写端
PTY 读端 ─→ vt.rs(Parser) ─→ Term ─→ grid.rs(paint) ─→ 窗口
鼠标/滚轮 ─→ app.rs ─→ 本地选区/滚动  或  keys.rs 鼠标上报
Term 事件 ─→ app.rs（标题、铃声、剪贴板、退出码）
```

线程模型：

- PTY 读线程：阻塞读 → `async_channel`（已依赖）→ GPUI 主线程。
- PTY 写：主线程直接写，短写循环，`EAGAIN` 重试。
- 子进程回收：`SIGCHLD` 由读线程 EOF + `waitpid` 处理，禁止在主线程 `wait`。
- 主线程不做阻塞 IO，不持有锁跨越 `await`。

记住一条：`app.rs` 里现在这套 `handle_key` 自行拼装按键判断，P2 之后要改成「先查自己的快捷键，未命中才把原始按键转成 PTY 字节」，否则 `Ctrl+R`、`Ctrl+P` 这些会被误吞。

## 6. 代码规范与结构约束

这一章是硬约束，和其他章节冲突时以这里为准。目的只有一个：不让任何文件在没人注意的时候长成几百上千行。

### 6.1 规模上限

| 对象 | 上限 | 超出后的做法 |
| --- | --- | --- |
| 单个 `.rs` 文件 | 400 行 | 拆成目录模块，`mod.rs` 只做导出与组装 |
| 纯视图文件 | 250 行 | 一个文件只放一个区域（顶栏、侧栏、标签栏…） |
| 单个函数 | 60 行 | 抽子函数，按「读数据 → 算布局 → 拼元素」三段拆 |
| `render` / 顶层视图函数 | 80 行 | 立刻拆成 `header()` / `body()` / `footer()` |
| 缩进深度 | 4 层 | 用 `?` / 早返回 / `match` 守卫压平 |
| `match` 分支单臂 | 10 行 | 臂体抽成命名函数 |
| 单文件内测试代码 | 文件的三分之一 | 移到 `tests/` 目录或拆分模块 |

配套的可执行门禁：新增 `scripts/check_structure.py`，扫描 `src/`，在 CI 里 fail：

```bash
python3 scripts/check_structure.py --max-lines 400 --max-fn-lines 60
```

另在 `lib.rs`/`main.rs` 打开严格 lint，禁止生产路径里的兜底：

```rust
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
```

`unwrap` 只允许出现在测试、已用 `matches!` 证明过的不可达分支，以及启动阶段的配置常量上，且必须写一行说明原因。

### 6.2 现有文件拆分目标

`views/` 和 `theme/` 已经拆好，作为参照标准：单个视图文件 60–180 行，`widgets.rs` 只是一组纯函数。剩下这几个文件需要按同样粒度处理。

P0 阶段必须完成，拆分后每个文件都在 400 行以内：

| 当前 | 行数 | 拆分后 |
| --- | --- | --- |
| `app.rs` | 1075 | `app/mod.rs`（结构与构造）、`app/session.rs`（会话仓储）、`app/keys.rs`（责任链）、`app/actions.rs`（Action + dispatch）、`app/palette.rs`、`app/prefs_ops.rs` |
| `dbusmenu.rs` | 696 | `io/dbusmenu/{mod,items,service,layout}.rs` |
| `tests.rs` | 316 | `tests/` 目录：`keys.rs` / `views.rs` / `menu.rs` / `session.rs` |
| `settings.rs` | 226 | 保留；但要拆出 `domain/shell_spec.rs`，shell 程序与参数归 domain |
| `preferences.rs` | 167 | `io/preferences/{mod,store,migrate}.rs` |
| `shell.rs` | 560 | P2 直接删除，删除前不再加功能 |
| `fs.rs` | 191 | P2 直接删除 |

已完成，不要回退：

| 已拆 | 结构 |
| --- | --- |
| `views.rs` (1818 行) | `views/{mod,widgets,sidebar,workspace,terminal,inspector,palette,preview}.rs` + `views/settings/` 五分文件；最大文件 172 行 |
| `theme.rs` (484 行) | `theme/{mod,colors,palettes,fonts,tests}.rs`；最大文件 199 行 |

注意 `views/widgets.rs` 现在用 `use super::*;`，展开后各子模块共享同一批 import。拆 `app/` 时不要照抄这一手法，按 6.3 明确列出各模块自己的 `use`。

### 6.3 分层与可见性

三层，靠模块可见性强制，不靠自觉：

```text
domain/   纯数据与规则：不能 use gpui、不能 use zbus
io/       PTY、配置读写、D-Bus：能 use zbus/serde，不能 use gpui
ui/       GPUI 视图与事件：能 use gpui，不能直接碰 std::process
```

- 跨层只通过 `app/actions.rs` 里的动作枚举和值类型通信，不传 `Entity`、不传 `Context`。
- `pub(crate)` 是默认，`pub` 只在真正对外的 API 上用。
- 新增一个「跨层 import」就跑不过 `scripts/check_structure.py` 的禁层检查。

### 6.4 模式落点

要求用模式，但只用在有真实变化点的地方。每条都给了 Rust 里的具体形态：

| 变化点 | 模式 | 形态 | 落点 |
| --- | --- | --- | --- |
| 按键处理会一路叠加（面板 → 设置页 → 快捷键 → 终端） | 责任链 | `trait KeyStage { fn handle(&mut self, ev: &KeyEvent, cx: &mut Ctx) -> Option<Handled>; }`，`Vec<Box<dyn KeyStage>>` 顺序尝试，`None` 交给下一环 | `app/keys.rs` 替换现在那个 120 行的 `match key` |
| 菜单、命令面板、快捷键要触发同一批动作 | 命令 | `enum Action` + 一个 `fn dispatch(&mut AppState, Action) -> Effect`，三处入口都只产出 `Action` | `app/actions.rs`；现在 `run_menu_action` / `run_palette_action` / `handle_key` 三套分支各写一遍，必须合并成一套 |
| 终端创建方式会变（本地 PTY、`-e` 直执行、将来的 SSH） | 工厂 | `fn spawn(spec: &SessionSpec) -> Result<Session>` 内部按 `SessionSpec::kind` 选实现；返回 `Box<dyn TerminalBackend>` 而不是 `dyn` 到处传 | `io/pty.rs` + `domain/session.rs` |
| shell 程序与参数按类型不同 | 工厂 + 枚举 | `enum ShellSpec { Default, Program(PathBuf), WithArgs { .. } }`，`fn command(self) -> std::process::Command` | `domain/shell_spec.rs`，替换 `ShellKind::prompt_marker/startup` |
| 按键序列编码有明确多套（普通 / 应用光标 / kitty 协议） | 策略 | `trait Encoder { fn encode(&self, ev: &KeyEvent, mode: TermMode) -> Option<Vec<u8>>; }`，三个零大小结构体，按 `TermMode` 选 | `io/keys/encoder.rs` |
| 配置要跨版本升级（v1 → v2 → …） | 责任链 | `fn migrate(v: u32, prefs: Value) -> Result<Value>`，每步一个函数，按版本顺序串起来，缺一步直接报错不可跳级 | `io/preferences/migrate.rs` |
| 配置存储要能换（文件 / 内存测试 / 只读） | 策略 | `trait Store { fn load(&self) -> Result<Preferences>; fn save(&self, ..) -> Result<()>; }` | `io/preferences/store.rs` |
| PTY 数据流、主题变化、菜单事件都要通知多处 | 观察者 | 已有 `async_channel` 的做法保留；主线程内改成「改一处 → 发一个 `Change` → 统一应用到菜单/视图」，禁止再见到散落的 `self.sync_menu()` | `app/mod.rs` 的 `apply(change)` |
| 会话有明确生命周期 | 状态机 | `enum SessionState { Starting, Running, Exited(ExitStatus), Failed(String) }`，禁止用 `Option<..>` + `bool` 拼状态 | `domain/session.rs` |
| 视图树层层嵌套 | 组合 | 保留 builder 风格，但每个可复用块（`card`、`row`、`field`、`section`）必须是独立函数，视图函数里不出现第四层缩进 | `ui/widgets.rs` |
| 会话集合的增删查 | 仓储 | `struct Sessions { items: Vec<Session>, active: usize }` + `add/remove/active_mut/next/prev`，`app` 只调这些方法，不直接索引 `Vec` | `app/session.rs` |

不作为可选建议，而是验收条件：P0 结束时，`app/keys.rs`、`app/actions.rs`、`io/preferences/store.rs` 三处必须按上表落地。P2 再加 `domain/shell_spec.rs` 与 `io/pty.rs`。

### 6.5 不要套模式的地方

反例同样重要，避免为了「有模式」把代码绕晕：

- **闭合集合不用 trait**。`ThemeId`、`MenuAction`、`Role` 都是固定枚举，加 trait 只会多一层间接。trait 只在实现数会增长时用。
- **不引入 DI 容器**。构造参数显式传递，`SiskinApp::new` 里组装一次即可。
- **不在热路径上用 `Box<dyn>`**。渲染和按键处理每帧都在跑，`dyn` 只出现在初始化阶段（后端、存储、编码器都只在启动时选一次，之后存具体类型或缓存好的 `Vec<u8>`）。
- **不为单个实现抽接口**。只有出现第二个真实实现（比如测试用的内存 `Store`）时才抽 trait。
- **不写多层 `impl` 转发**。`impl A { fn x(&self) { self.b.x() } }` 这种纯委托要么删掉，要么说明清楚边界在哪。

### 6.6 函数与模块的写法

- 一个函数只做一件事，名字说清做的事：`render_tab_bar`、`encode_key`、`apply_change`，不用 `handle`、`process`、`do_*` 这种含糊词（除非是实现 trait 的必需方法）。
- 参数超过 4 个就打包成结构体，`palette` + `font_size` + `row_height` + `columns` 这类每帧都在传的，用 `render::Metrics` 一次带过去。
- 视图函数不计算业务状态，只读 `app` 的查询方法；需要派生值的，在 app 层提供 `fn rows(&self)` 这类方法。
- 不写 `String` 拼路径、拼命令；用 `PathBuf` / `OsString`。
- 事件处理里不做 IO。需要写 PTY 的，把 `Vec<u8>` 交给写队列，由单一写线程负责。

### 6.7 文件头与注释

沿用仓库现有风格，新建文件一律加头：

```rust
// @Time    : 2026/11/5 10:55
// @Author  : fzf
// @FileName: keys.rs
// @Software: Siskin
```

注释只写代码看不出来的东西：

- 写「为什么」：为什么这个序列要加 `ESC` 前缀、为什么 resize 要先算行列再调 `TIOCSWINSZ`。
- 不写「是什么」：`// 遍历 cells` 这种删掉。
- 不写任务与 PR 上下文：`// fix bug #12` 不进代码。
- 每个 `unsafe`（`x11rb`、PTY fd 操作）必须有一条 `// SAFETY:` 说明前提条件。

### 6.8 错误处理

- domain 层用具体错误枚举（`enum SessionError { Spawn(io::Error), Resize(..) }`），不用 `Box<dyn Error>` 传染。
- 只在外层（UI 事件、启动流程）把错误转成用户可读的中文提示 + 一行日志。
- 禁止吞错：`let _ = ...` 只允许在「失败不影响主流程且已有日志」的地方，且要写注释。
- 子进程、PTY、文件句柄都有明确的释放点，不依赖 drop 顺序的巧合。

### 6.9 测试与验证

- 每加一个 `match` 分支或一条按键序列，就加一条对应单元测试；按键编码用「输入 → 期望字节」的表格驱动测试。
- 视图拆分时同步迁移 `debug_selector`，`tests.rs` 里的点击路径不能因为拆分失效。
- 性能相关改动（整形缓存、背景合并）必须附一个可复现的 bench 或计数断言，不接受「感觉变快了」。

## 7. 分阶段计划

按单人节奏估算，含验证时间。每阶段结束都要能跑起来、能演示，不允许「半截编译不过」。

### P0 结构与门禁（约 3 天）

目的：先把房子拆成合理的房间，再往里搬新家具。`views/` 与 `theme/` 已拆好，这一阶段补齐剩下的，并建立门禁防止回退。

范围：

- 新增 `scripts/check_structure.py`：文件行数、函数行数、缩进深度、跨层 import（`domain` 不许 use gpui/zbus，`io` 不许 use gpui，`ui` 不许 use `std::process`）、`unwrap/expect` 白名单，CI 里跑。
- 拆分 `app.rs`（1075）与 `dbusmenu.rs`（696），按 6.2 的目标结构建 `app/` 与 `io/dbusmenu/`。
- 建 `domain/` 与 `io/` 两个骨架目录，先把 `settings.rs` 的 shell 部分挪进 `domain/shell_spec.rs`，`preferences.rs` 挪进 `io/preferences/`。
- 按 6.4 落地 `app/actions.rs`（命令模式）与 `app/keys.rs`（责任链），把现在 `run_menu_action` / `run_palette_action` / `handle_key` 三套分派合并成一套。
- `io/preferences/` 抽 `Store` trait，提供文件与内存两种实现，测试不再落真实临时目录。
- 打开 `deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)`，逐个清理生产路径。
- 现有 `tests.rs` 迁到 `tests/` 目录，`debug_selector` 全量保留。`views/` 各文件里的 `#[cfg(test)]` 一并迁出。

验收：

- `python3 scripts/check_structure.py` 通过，最大文件 ≤ 400 行，`domain/` 与 `io/` 无 GUI 依赖。
- 四道检查和全部 GUI 测试通过，拆分前后行为一致（用 `docs/` 里现有的截图做比对基线）。
- 新增一个动作（比如「切换 inspector」）只需要在 `actions.rs` 加一个枚举项，不再改三处 `match`。
- `preferences` 的测试不再调用 `tempfile::tempdir()`。

### P1 渲染内核验证（约 4 天）

目的：先证明 GPUI 能画出正确的终端网格，再谈 PTY。

范围：

- 新增 `ui/grid.rs`：自定义 `Element`，输入是假的 cell 矩阵。
- 新增 `io/ansi.rs`：ANSI 名字色、256 色、truecolor 到 `Palette` 的映射。
- 用 `alacritty_terminal` + `vte::Parser` 离线解析一份固定 ANSI 样例（不接 PTY）。
- 处理：粗体/斜体/下划线/反显/隐藏、宽字符、组合字符、光标四种形状（块/竖线/下划线/空心）。
- 尺寸：由字体度量推 `cell_width`/`row_height`，不是魔数 `0.6 em`。

验收：

- 与 `alacritty` 同一样例截图逐格比对，字符位置、颜色、换行一致。
- 中文文案不乱宽度，emoji 不挤掉后一列。
- 2000 行滚动时只整形可视行，单帧整形次数可统计。

风险处置：如果 `ShapedLine::paint` 无法按 run 精确着色，退回到逐 cell `paint_glyph`，但必须带整形缓存，先用数据说话。

### P2 PTY 与会话生命周期（约 4 天）

范围：

- 新增 `io/pty.rs`、`domain/session.rs`、`io/vt.rs`；删除 `shell.rs`、`fs.rs` 及其调用。P0 已经建好这两个目录，这里往里填实现。
- shell 选择改为「真实可执行文件路径 + 参数」，`ShellKind` 只作为预设之一；`$SHELL` 为缺省值。
- `Term::resize` 与 `TIOCSWINSZ` 同步，窗口尺寸变化时先算新行列数再 resize，避免回环。
- 子进程退出：保留最后输出，标签显示退出码，`Enter` 关闭会话。
- `Session::new` 不再预置三个假会话，改成配置恢复或单会话启动。
- P0 拆出的 `app/session.rs` 在这里补仓储方法，`app` 不再直接索引 `Vec<Session>`。

验收：

- `vim` 打开按 `q` 能退，`htop` 布局正常，`tmux` 分屏与状态栏正确。
- `ssh localhost` 能进，`Ctrl+C`、`Ctrl+Z`、`Ctrl+D` 行为正确。
- 拖动窗口边界时 `stty size` 跟着变，输出不错行。

### P3 输入法、选区与剪贴板（约 4 天）

范围：

- `EntityInputHandler` 全量实现，预编辑串内联渲染。
- 选区：拖选、双击选词、三击选行、`Shift+Click` 扩展、选区高亮带背景色。
- 剪贴板：`Ctrl+Shift+C/V`、`Ctrl+Shift+A` 全选、中键粘贴 primary。
- OSC 52：默认只允许「写剪贴板」，禁止「读剪贴板」，配置可放开。
- 粘贴时把 `\r\n` 与单独的 `\n` 统一成 `\r`（终端里回车才是换行），多行粘贴在 `TermMode::BRACKETED_PASTE` 开启时包上 `ESC[200~` … `ESC[201~`，避免读到一半就执行。

验收：

- fcitx5 / ibus 下中文组词、选词、退格、候选框跟随光标。
- 从浏览器粘三行命令，粘贴后不会自动执行。
- 复制选区得到的就是屏幕上的字，宽字符不串位。

### P4 交互补全（约 4 天）

范围：

- 滚轮按行滚动、`Shift+PageUp/PageDown` 翻页、`Shift+Home/End` 到顶/底、输入后贴回底部。
- 搜索：`Ctrl+Shift+F` 打开搜索条，正则高亮、上一个/下一个、Esc 关闭。
- 链接：`Ctrl+Click` 打开，识别 `http(s)://`、`file://`、`path:line:col`，相对路径以会话 cwd 解析；协议白名单之外不打开。
- 字号（`Ctrl +/-/0`）、行高、padding 实时生效并立即重排。
- 命令面板换真实条目：新窗口、关闭会话、搜索、复制、清屏、设置、重置字号。

验收：一整天真实使用（编辑、编译、看日志、连远程）不出现必须切回旧终端的情况。

### P5 持久化、多窗口与桌面集成（约 4 天）

范围：

- 配置读写（见 4.5、6.4 的 `Store` 与迁移责任链），设置页改成「改动即写、失败回滚 UI」。
- 会话恢复：记 cwd 与标签名，不记输出；可选「恢复上次会话」。
- 单实例 + 新窗口（见 4.6）。
- 菜单与快捷键对齐，Wayland 下隐藏全局菜单、保留窗口内菜单。
- 右侧 inspector（`views/inspector.rs`，目前写的是「当前为界面预览，未连接真实 PTY」）换成真实 cwd / git 分支 / 子进程状态 / 会话尺寸；标题栏 `simulated` 字样去掉。
- 侧栏会话行展示真实状态（运行中 / 已退出 / 启动失败），不再共用同一个静态图标。

验收：

- 改主题、字体、shell 后重启仍在。
- 第二次 `siskin` 激活已有窗口，不新起进程。
- 全局菜单里的主题与 shell 状态与窗口一致。

### P6 打包与图标（约 3 天）

范围：

- 图标换新：已有设计稿是 1024/512/256/180/128/64/32/16 的位图集合（`>_` 提示符 + 黄色小鸟）。需要补一个 SVG 母版（`assets/siskin.svg`）作为可缩放源，其余尺寸从母版导出，不靠位图缩放，否则 16px 会糊。
- 按 hicolor 目录安装：`scalable/apps/io.github.fzf.Siskin.svg` 与 `16/22/24/32/48/64/96/128/256/512` 的 `1x`、`2x` PNG（22/24/48/96 由母版新导，现有集合没覆盖）。
- 窗口内嵌图标（`main.rs` 的 `window_icon`）同步换成 256 的新稿。
- `packaging/io.github.fzf.Siskin.desktop`：`Exec=siskin`、`Icon=io.github.fzf.Siskin`、`StartupWMClass=io.github.fzf.Siskin`、`Categories=System;TerminalEmulator;`、中文 `Name`/`Comment`/`Keywords`。
- `scripts/build_deb.py` 参照 `resource-pulse` 重写：多尺寸图标、`postinst`/`postrm` 刷新图标缓存、`dpkg-shlibdeps` 自动依赖。
- README 重写：从「模拟终端」改成真实终端，写清算什么不支持。
- CHANGELOG 从 0.2.0 开始记录。

验收：

- 干净容器里 `dpkg -i` 后开始菜单出现、Plank 能固定、DDE 顶栏菜单在 X11 下正常。
- 直接从构建目录跑二进制，窗口图标与任务栏图标也是新的。
- 卸载后不留图标缓存残留。

### P7 稳定性与性能收尾（约 4 天）

范围：

- 基准：10 万行 scrollback 下滚动帧时间、打字延迟、内存曲线（8 小时）。
- 长输出压测：`seq 1 1000000`、`yes | head -n 500000` 不卡死、不丢行。
- 异常路径：PTY 派生失败、shell 不存在、D-Bus 不可用、配置损坏、磁盘只读，全部有可读提示且能继续用。
- 日志分级别，默认只写 `warn` 以上到 stderr，不落敏感内容。
- 文档：快捷键、配置项、已知限制。

验收：目标 60fps 滚动、按键到显示 < 16ms、8 小时常驻内存增长可解释。

## 8. 关键设计决策

| 决策 | 选择 | 理由 | 放弃的方案 |
| --- | --- | --- | --- |
| VT 解析 | `alacritty_terminal` | 成熟、被 Zed/Alacritty 长期验证 | 自研 vte、`termwiz`（API 更碎） |
| 渲染 | 自定义 `Element` + 整行整形 | 一处实现，性能可控 | 逐 cell `div`（元素爆炸）、WebView（违背项目定位） |
| 快捷键 | 终端动作统一 `Ctrl+Shift+*` | 避开 shell 与 readline 常用键 | `Ctrl+C` 抢作复制（破坏习惯） |
| 配置格式 | JSON | 无需新解析依赖，容错好 | TOML（多一个 crate）、YAML（缩进易错） |
| 会话恢复 | 只恢复 cwd 与标签 | 避免伪造历史，实现简单 | 保存完整输出（体积与语义都有问题） |
| 主题 | 保留 `Palette`，补 ANSI 表 | 现有 `theme/` 已支持预设 + 自定义取色，直接复用 | 引入 Alacritty 主题文件格式（多一套兼容） |
| 全局菜单 | 保留 D-Bus 实现 | 已跑通，DDE 体验需要 | 窗口内菜单栏（占高度） |

## 9. 风险与未验证项

按影响排序，每条都写处置方式：

1. **网格渲染性能与字形度量**（高）：GPUI 没有终端渲染先例，宽字符与连字的度量可能偏差。P1 必须先出可行 demo，否则整个方向要重新评估。
2. **Wayland 输入法**（高）：`text-input-v3` 支持依赖 GPUI 与合成器实现。P3 单独验收；不可用时明确写进 README，并提示切 X11。
3. **`alacritty_terminal` API 稳定性**（中）：0.26.0 是发布版，但选区/链接接口在 minor 之间会动。隔离在 `io/vt.rs`，升级只改一个文件。
4. **PTY 信号与子进程回收**（中）：`SIGCHLD`、控制终端、`setsid` 顺序错了会出现僵尸或 `EIO`。P2 用 `sh -c 'trap ...'` 系列用例覆盖。
5. **打包 ABI**（中）：Deb 只能在构建机同代发行版安装。文档写明，不承诺跨发行版。
6. **OSC 52 与链接安全**（中）：默认收紧策略，配置可放开，文档写清风险。
7. **用 `pulse-ui` 的代价**（低）：绑定 `v0.1.1` 与固定 GPUI commit。需要新组件时先在 `pulse-ui` 实现再升 tag，不在 Siskin 里分叉。

## 10. 验证矩阵

每个 PR 必过：

```bash
cargo fmt --all --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --all-targets --locked
cargo build --locked
neo-runner run -f .agents/loop.yaml --output json
python3 scripts/check_structure.py --max-lines 400 --max-fn-lines 60
```

前五条现在就在跑（`.agents/loop.yaml` 已串好顺序）。最后一条从 P0 起生效，并追加进 `loop.yaml` 作为新的最后一环。

测试分层：

| 层 | 覆盖 | 运行环境 |
| --- | --- | --- |
| 单元 | VT 状态机用例、ANSI 颜色解析、键序列编码、尺寸取整、配置迁移 | 无显示器 |
| 集成 | 真 PTY 起 `sh -c` 跑命令，断言读到的输出与退出码 | 无显示器 |
| 协议 | D-Bus 起独立总线，校验方法调用、子菜单深度、属性 | 无显示器（现有做法） |
| GUI | `TestAppContext` 模拟按键/点击/粘贴，断言屏幕状态 | 无显示器（现有 `tests.rs` 模式） |
| 实机 | `scripts/check_desktop.py` 校验窗口、菜单注册、图标；`scripts/capture_window.py` 抓像素比对 | X11 桌面 |

手动清单固化进 `docs/manual-checklist.md`，每次发版执行一遍。现有 `scripts/` 下两个脚本已经是实机验收的一部分，P6 的打包验收直接复用它们。

## 11. 立即可做的三件事

1. 打 tag：`0956206`（feat: redesign Siskin as a focused terminal workspace）已经是可回退的基线，54 项检查通过，打 `v0.1.0-demo` 标记它就够了。
2. 先做 P0：写 `scripts/check_structure.py`，然后拆 `app.rs`（1075 行）、`dbusmenu.rs`（696 行），建 `domain/` 与 `io/` 骨架。这一步不加任何功能，拆完行为必须完全一致。`views/` 与 `theme/` 已经拆好，直接作为参照。
3. 起一个 `spike/grid-render` 分支，只做 P1：假 cell 矩阵 + `Element` + ANSI 样例截图比对，不碰 PTY，两天内出结论。

图标整理（P6 的一部分）可以和 P0 并行，因为 P1 的视觉比对需要统一视觉基线。

## 12. 与现有代码的关系

基线是 `0956206`，工作区干净。这一版已经落地了：版本化配置（原子写入、损坏不覆盖）、`Paper/Graphite` 中性配色、自定义取色、分区设置页、D-Bus 全局菜单、`views/` 与 `theme/` 的目录拆分。后续按下面的顺序推进：

1. P0 拆分 `app.rs` 与 `dbusmenu.rs`，`preferences.rs` 同时抽成 `io/preferences/{mod,store,migrate}.rs`，`Store` trait 提供文件与内存两种实现，测试不再落真实磁盘。
2. P2 删除 `shell.rs`/`fs.rs` 时，设置页里的 shell 预设改为 `domain/shell_spec.rs` 的路径候选，`io/dbusmenu/` 的 `MenuAction::SetShell` 跟着改，不保留旧语义。
3. `ui/` 的会话栏、工作区、状态栏、设置页保留，只替换终端卡片内部；`theme/` 只增不改，避免设置页回归。
4. `docs/` 下已有 light / dark / inspector / settings 四张验收截图，P0 拆分与 P1 渲染都用它们做像素比对基线，不要重新拍一套。
