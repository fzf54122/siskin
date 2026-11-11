<!--
@Time    : 2026/11/11
@Author  : fzf
@FileName: glass.md
@Software: Siskin
-->

# 磨砂玻璃

AI 终端 UI 重设计的第五根支柱：Apple 风格玻璃质感。桌面效果待大王在
Linux 桌面上确认（云端 VM 无显示器/Vulkan，见末尾「未验证项」）。

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

所以本支柱的实现是**半透明填充 + 细边框 + 分层透度**的近似，并把 GPUI
的窗口级 `Blurred` 也接上：在 KDE Plasma Wayland 上，窗口背后的桌面
会叠加一道真模糊；GNOME Wayland 与 X11 上是同样的半透明外观，只是
背后不模糊。没有硬编任何做不到的效果。

## 做了什么

- **玻璃面**（半透明底 + 1px 边框 + 圆角，终端主区不动）：
  - 命令面板（整屏遮罩上的 580px 面板）
  - 搜索条（浮在终端右上角）
  - Agent 侧栏、会话信息检查器、设置页左侧导航栏（dock 的 chrome）
  - 弹窗：关闭确认、底部状态横条、性能采样浮层
  - 整窗画布：按玻璃透度半透明（之前写死 0.92）
- **终端主区保持实底**：每张终端卡片仍是 `palette.terminal` 不透明底，
  输出文字不经过任何半透明层。
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
（单层玻璃直接盖纯黑/纯白桌面）：

| 场景 | 背后那层 | 验算的文字 |
|---|---|---|
| 浮层（命令面板、搜索条、弹窗） | 终端底；命令面板与弹窗多一层整屏遮罩 | ink / muted / accent / error |
| 侧栏（Agent 侧栏、设置导航栏） | 画布 + 桌面（画布本身同透度透出桌面，黑白两端桌面都要算） | ink / muted |
| 兜底 | 纯黑 / 纯白桌面 | ink |

注意边界：自定义主题的极端配色（例如深终端底配很淡的次要文字）
在玻璃开启且桌面背景反差极大时，次要文字仍可能低于 4.5:1——
自定义主题的 `muted`/`accent` 本来就不像 `ink` 那样经过对比度校正；
主文字有校正打底，仍在安全区内。

## 平台效果矩阵

| 桌面 | 玻璃开启时 |
|---|---|
| KDE Plasma（Wayland） | 浮层半透明 + 窗口背后真模糊（KWin blur 协议） |
| GNOME（Wayland） | 浮层半透明，背后不模糊（Mutter 不支持该协议） |
| X11（任意合成器） | 浮层半透明，背后不模糊（GPUI 不发模糊请求） |
| 无合成器的 X11 | 透明区域按合成器缺失处理，与玻璃之前（窗口已是 `Transparent`）一致；关掉玻璃可回实色 |

## 未验证项

- 桌面实际观感（模糊是否生效、半透明在真实壁纸上的效果）：
  云端 VM 无显示器/Vulkan 测不了，待大王在 Linux 桌面上确认，
  重点看 KDE 下的真模糊与 GNOME/X11 下的半透明。
- 自定义主题的极端配色 + 玻璃 + 高反差桌面组合（见上节边界说明）。

## 相关代码

- `src/theme/glass.rs`：透度常量、填充色合成、对比度验算与单测。
- `src/settings.rs`：`glass` / `glass_opacity` 字段与 `glass_fill`。
- `src/app/glass_ops.rs`：开关、透度循环、窗口背景同步。
- `src/io/preferences/mod.rs`：`glass` / `glass_opacity` 持久化。
- `src/ui/settings/appearance.rs`：设置页入口。
- 命令面板入口：`src/app/actions.rs`（`ToggleGlass` / `CycleGlassOpacity`），
  快捷键 Ctrl+Shift+G（`src/app/keys.rs`）。
