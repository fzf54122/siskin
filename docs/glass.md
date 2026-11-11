<!--
@Time    : 2026/11/11
@Author  : fzf
@FileName: glass.md
@Software: Siskin
-->

# 磨砂玻璃

AI 终端 UI 重设计的第五根支柱：Apple 风格玻璃质感。桌面效果待大王在
Linux 桌面上确认（云端 VM 无显示器/Vulkan，见末尾「未验证项」）。

2026-11-11 修复：大王在桌面上拉了远程 176a75705 跑，说“玻璃质感还是
没有”。原因是玻璃只做在浮层上、终端主区实底，浮层后面就是终端自己的
底色，同色系下 0.85 透度几乎看不出区别——效果弱到看不见，不是没生效。
这次把玻璃做到整窗：终端底也透出桌面壁纸，浮层再加浓。

## 先说结论：真模糊做不了，玻璃是近似

核实过 vendored GPUI（`~/.cargo/git/checkouts/zed-*/c427327/crates`，
rev `c427327`）与 pulse-ui v0.1.1 的源码，结论如下：

| 层面 | 结论 |
|---|---|
| GPUI 逐元素 backdrop blur | **没有**。`scene.rs` 的 `blur_radius` 只服务于阴影；`div` 背景只有纯色填充与 alpha 混合，没有类似 CSS `backdrop-filter` 的接口。浮层底下的终端内容无法在 GPUI 内被模糊。 |
| GPUI 窗口级 `WindowBackgroundAppearance::Blurred`（`gpui/src/platform.rs`） | **有，但只管窗口背后**（桌面壁纸/其它窗口），不管窗口内元素。 |
| Wayland 下的 `Blurred`（`gpui_linux/src/linux/wayland/window.rs`） | 走 KDE 的 `org_kde_kwin_blur` 协议（`wayland_protocols_plasma::blur`），`blur_manager` 是 `Option`，合成器不支持时静默无事发生。**只有 KWin（KDE Plasma）实现该协议**，GNOME/Mutter 不支持，退化为普通透明。 |
| X11 下的 `Blurred`（`gpui_linux/src/linux/x11/window.rs`） | 与 `Transparent` 等价：只把渲染器切到透明模式，**不向合成器发任何模糊请求**（没有 `_KDE_NET_WM_BLUR_BEHIND_REGION` 之类的处理），picom/KWin-X11 都不会因此模糊。 |
| pulse-ui v0.1.1 | 没有任何 blur / backdrop / glass 接口（全文 grep 无命中）。 |

所以本支柱的实现是**整窗半透明 + 磨砂 tint + 亮边/顶光/阴影**的
近似，并把 GPUI 的窗口级 `Blurred` 也接上：在 KDE Plasma Wayland
上，窗口背后的桌面会叠加一道真模糊；GNOME Wayland 与 X11 上是同样
的半透明外观，只是背后不模糊。没有硬编任何做不到的效果。

## 做了什么

- **整窗玻璃**（2026-11-11 修复后）：终端主区底色也参与玻璃——
  每张终端卡片的 div 底与网格自身的底色矩形都按终端透度半透明，
  桌面壁纸从整窗透出来（Ghostty 的 background-opacity 那种做
  法），而不再是只有 chrome 透、主区实底。终端底不低于 0.92
  不透明度（见下节），文字、选区、光标仍不透明画在上面。
- **玻璃面**（半透明底 + 磨砂 tint + 1px 亮边 + 顶部高光线 +
  柔和阴影 + 圆角）：
  - 命令面板（整屏遮罩上的 580px 面板）
  - 搜索条（浮在终端右上角）、回到底部按钮
  - Agent 侧栏、会话信息检查器、设置页左侧导航栏（dock 的 chrome）
  - 弹窗：关闭确认、底部状态横条、性能采样浮层
  - 整窗画布：按玻璃透度半透明（之前写死 0.92）
- **磨砂 tint**：深色主题的浮层底往白提 10%（如 0x1e222b →
  0x353840），肉眼可见的磨砂感；浅色主题不压暗——Paper 的浮层
  报错文字在 0.85 档本来就只有 4.58:1，任何压暗都会跌破 4.5:1，
  浅色玻璃的“边”靠亮边、顶光与阴影来读。
- **亮边与顶光**：深色主题用 32% 白亮边、浅色用 18% 黑细边；
  每块浮层顶边内侧一条 1px、35% 白的高光线；浮动面板（命令面板、
  搜索条、关闭确认、状态横条、回到底部按钮）加 `shadow_lg` 柔和
  阴影。dock 在边上的侧栏/检查器/导航栏不加阴影。
- **窗口级背景跟随开关**：玻璃开 → `WindowBackgroundAppearance::Blurred`
 （KDE 上真模糊，其余平台等价于之前的 `Transparent`）；玻璃关 →
  `Opaque`，合成器不用再画窗口背后的内容。只在开关变化时调用一次，
  不每帧重复触发平台往返。
- **特意没做玻璃的地方**：
  - 顶部的会话 tab 条保持实色：它上面有 `faint` 颜色的目录文字，
    浅色主题的 `faint` 在纯白底上本来就只有约 4.65:1，半透明盖在深色
    桌面上会跌到约 4.1:1，保不住对比度。
  - 设置页主区域、终端卡片、按钮与小提示（tooltip）保持实色：
    主内容区不透是设计原则，按钮实色落在玻璃面上是正常的分层做法。
    注意：这里的“终端卡片”指卡片里的文字内容层，卡片底色本身
    现在是半透明的（见上）。

## 开关与透度

默认：**开启，透度 0.85**（最通透且仍满足对比度的一档；见下节）。

- 设置页 → 外观 →「玻璃效果」：开/关按钮、「玻璃透度」三档按钮
  （标准 0.85 / 较浓 0.90 / 浓郁 0.95）。
- 命令面板（Ctrl+K）：「玻璃效果开/关」「玻璃透度」两条。
- 快捷键：Ctrl+Shift+G 开/关玻璃。
- 透度只影响已开启的玻璃；关闭时所有浮层回到实色，与玻璃之前
  （0.92 画布时代之前）的外观一致。
- 设置随 `settings.json` 落盘（`glass`、`glass_opacity` 字段）：
  旧配置文件没有这两个字段时默认开启 + 0.85；手改越界的透度在
  读取时钳制，不拒绝整份配置。
- 透度档管浮层与画布，**终端底另有 0.92 下限**：0.85/0.90 两档
  的终端底都按 0.92 画，0.95 档按 0.95 画（见下节验算）。

## 接线核实（2026-11-11，修效果前先查的线）

“还是没有”先排除了接线问题，再谈效果弱：

- 默认开：`Settings::default` 的 `glass: true`、透度 0.85。
- 旧配置缺字段：`Preferences` 的 `glass` / `glass_opacity`
  用 `serde(default)` 回默认开 + 0.85，有单测覆盖。
- 开关三路都生效且落盘：设置页外观、命令面板、Ctrl+Shift+G，
  末尾都走 `commit_settings`（菜单同步 + 标记脏，500ms 防抖
  后写盘，退出前强制 flush）。
- `WindowBackgroundAppearance`：每帧 `render` 开头调
  `sync_glass_appearance`，只在开关变化时推一次；初始
  `last_glass_appearance` 为 `None`，首帧即应用。
- 结论：**没发现“默认没开”或“设置没落盘”这类接线 bug**，
  不用修接线，问题就是浮层玻璃太弱 + 主区实底。

## 可读性：对比度怎么保住的

阈值与主题校正用同一把尺：WCAG 4.5:1（`theme::MINIMUM_CONTRAST`）。

透度下限 0.85 是算出来的，不是选出来的。对六套预设、三档透度逐套
验算（`src/theme/glass.rs` 的 `glass_text_stays_readable_in_every_preset`）：

- 0.75 时 Paper 的浮层文字只有 4.34:1、Moss 的侧栏次要文字只有
  4.30:1，都跌破 4.5:1；
- 0.80 时 Paper 只剩 4.50，零余量；
- 0.85 起最差组合（Paper 的报错文字盖在遮罩压暗后的终端底上）
  为 4.67:1，才有余量。

验算按玻璃后面那层分三档场景，主文字（ink）另加「任意底色」兜底
（单层玻璃直接盖纯黑/纯白桌面）。整窗玻璃后又加了两档新验算，
都在 `src/theme/glass.rs` 的单测里：

| 场景 | 背后那层 | 验算的文字 |
|---|---|---|
| 浮层（命令面板、搜索条、弹窗） | 终端底；命令面板与弹窗多一层整屏遮罩 | ink / muted / accent / error |
| 侧栏（Agent 侧栏、设置导航栏） | 画布 + 桌面（画布本身同透度透出桌面，黑白两端桌面都要算） | ink / muted |
| 兜底 | 纯黑 / 纯白桌面 | ink |
| **终端文字（新增）** | 纯黑 / 纯白桌面（终端底按档位换算后的实际透度） | ink / muted |
| **叠加最坏情况（新增）** | 浮层 0.85 + 整屏遮罩 + 终端底 0.92 + 纯黑/纯白桌面 | ink / muted / accent / error |

终端下限 0.92 的算法：`muted` 在 0.85 黑桌面下只有 4.08:1
（Paper/Moss），0.90 时 4.61 余量仍薄，0.92 起最差组合
（Paper 的 `muted` 盖黑桌面）为 4.83:1。没有为保 0.85 而砍档：
0.85 档对浮层是成立的，只对终端不成立，所以终端底单独抬下限，
浮层三档不动。叠加最坏情况 Paper 的报错文字 4.58:1，余量薄但
仍在 4.5 之上——这是目前最紧的余量，动透度或 tint 前先重算。

注意边界：自定义主题的极端配色（例如深终端底配很淡的次要文字）
在玻璃开启且桌面背景反差极大时，次要文字仍可能低于 4.5:1——
自定义主题的 `muted`/`accent` 本来就不像 `ink` 那样经过对比度校正；
主文字有校正打底，仍在安全区内。ANSI 命名色（如深色主题的黑色
0x1b1e25）本来在实底上就与终端底接近，不纳入玻璃验算范围——
那不是玻璃引入的回归。

## 平台效果矩阵

| 桌面 | 玻璃开启时 |
|---|---|
| KDE Plasma（Wayland） | 整窗半透明 + 窗口背后真模糊（KWin blur 协议），终端底也透出模糊后的壁纸 |
| GNOME（Wayland） | 整窗半透明，背后不模糊（Mutter 不支持该协议） |
| X11（任意合成器） | 整窗半透明，背后不模糊（GPUI 不发模糊请求） |
| 无合成器的 X11 | 透明区域按合成器缺失处理，与玻璃之前（窗口已是 `Transparent`）一致；关掉玻璃可回实色 |

## 未验证项

- 桌面实际观感（模糊是否生效、半透明在真实壁纸上的效果）：
  云端 VM 无显示器/Vulkan 测不了，待大王在 Linux 桌面上确认，
  重点看 KDE 下的真模糊与 GNOME/X11 下的半透明，**尤其看整窗
  玻璃之后终端底透出壁纸时字是否还清楚**（单测只算了纯黑/纯白
  两端桌面，真实壁纸是杂色）。
- 叠加最坏情况余量薄：Paper 浮层报错文字 4.58:1，真实壁纸下
  可能比纯黑桌面更严，桌面上若看不清先关一档透度或关玻璃。
- 自定义主题的极端配色 + 玻璃 + 高反差桌面组合（见上节边界说明）。

## 相关代码

- `src/theme/glass.rs`：透度常量、填充色合成、磨砂 tint/亮边/
  终端下限、对比度验算与单测。
- `src/settings.rs`：`glass` / `glass_opacity` 字段与 `glass_fill` /
  `glass_surface` / `glass_terminal_fill` / `terminal_bg_alpha`。
- `src/app/glass_ops.rs`：开关、透度循环、窗口背景同步。
- `src/io/preferences/mod.rs`：`glass` / `glass_opacity` 持久化。
- `src/ui/grid/`：终端底色矩形按 `bg_alpha` 画（`inputs.rs` /
  `mod.rs` / `element.rs`）。
- `src/ui/widgets.rs`：`glass_top_highlight` 与 `glass_edge`。
- `src/ui/settings/appearance.rs`：设置页入口。
- 命令面板入口：`src/app/actions.rs`（`ToggleGlass` / `CycleGlassOpacity`），
  快捷键 Ctrl+Shift+G（`src/app/keys.rs`）。
