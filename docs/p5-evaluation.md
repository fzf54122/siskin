<!--
@Time    : 2026/11/11
@Author  : Lux
@FileName: p5-evaluation.md
@Software: Siskin
-->

# P5 评估记录（2026-11-11）

> **性质：只评估，不实现。** 本文档不对应任何代码改动；
> plan.md §9 的三项能力在本次评估后仍全部处于「暂缓」状态，
> 结论栏的「可考虑」只表示「下一轮复评时的候选顺序」，不是立项。
> 现状部分只写本次在代码里实际看到的，附文件路径；
> 外部事实均在 2026-11-11 用搜索核实，附来源，未查到的写明未查到。

## 0. 仓内现状盘点（评估基准：本地 `main`，测试基数 269）

### 0.1 SSH：没有任何内置 SSH 代码

- 全仓 grep `ssh|russh|ssh2|keyring|secret-service`（`src/`、`Cargo.toml`、
  `docs/`、`plan.md`、`scripts/`、`packaging/`），除注释与文档外
  **零命中**：`Cargo.toml` 与 `Cargo.lock` 里没有 `russh`、`ssh2`、
  `keyring`、`secret-service` 或任何 SSH 相关依赖。
- 仅有的三处 `ssh` 字样都不是功能：
  - `src/app/events.rs:300` 注释：举例说 `vim`/`ssh` 改 OSC 标题时标签保持稳定；
  - `src/app/tab_ops.rs:54` 注释：同上，会话名跟随工作目录的说明；
  - `docs/manual-checklist.md:29`：手工检查项「`ssh localhost` 能登录」，
    指用户在终端里自己运行系统 `ssh` 命令。
- PTY 只派生本地 shell：`src/io/pty.rs:63-77`（`Pty::spawn`，
  `ShellSpec` 解析本地可执行文件路径）；没有「远程会话」这一会话类型，
  `Session` 没有 host/port/认证字段（`src/app/session.rs`）。
- 今天的实际状态：**SSH 靠用户在终端里运行系统 `ssh` 二进制**，
  CHANGELOG 0.2.0 也只说「支持 `ssh`」作为子程序。密钥、known_hosts、
  连接复用全部由系统 OpenSSH 负责，Siskin 完全不碰。

### 0.2 Kitty Graphics / Sixel：解析层丢弃，渲染层没有图像位置

- 依赖固定：`alacritty_terminal =0.26.0`、`vte =0.15.0`
  （`Cargo.toml:29`，`Cargo.lock` 的 `vte` 条目）。
- 全仓 grep `kitty|sixel|graphics`：`kitty` 的命中只有 **Kitty 键盘协议**
  （`src/io/vt/mod.rs:32`，`TermMode::KITTY_KEYBOARD_PROTOCOL`；
  `src/domain/key.rs:14` 的 `kitty_keyboard` 字段；上游
  `alacritty_terminal-0.26.0/src/term/mod.rs` 同名模式位）。
  **没有一处**提到 Kitty 图形协议、Sixel 或图像 cell。
- 上游行为（查 vendored 源码确认，不是凭印象）：
  `vte 0.15.0` 的 `Perform` trait 的 `hook` / `put` / `unhook`
  （DCS，即 Sixel 的载体）默认是空实现（`src/lib.rs:779-789`）；
  `alacritty_terminal 0.26.0` 的 `Term` 实现的 `Handler` trait
  （`src/term/mod.rs:1059`）没有任何图像 / DCS / APC 方法，
  只有 `input`、`goto`、`osc_dispatch` 这类文本与 OSC 方法。
  结论：**Kitty 图形（APC `G`）与 Sixel（DCS `q`）的字节能被
  vte 状态机解析、不破坏终端状态，但随后被静默丢弃**——程序发图，
  用户什么也看不到，也不会乱码或崩溃。
- Siskin 自己的帧里没有图像的位置：`Frame { rows, cursor,
  display_offset }`，`CellView { text, style, spacer, selected,
  link }`（`src/io/vt/view.rs:30-56`），没有图像 id、像素、占位 cell；
  网格绘制只画文本 run 与选区背景（`src/ui/grid/paint.rs`），
  没有图像层。`image =0.25.10`（仅 `png` feature）全仓只用在一处：
  `src/main.rs:110-112` 解码窗口图标，与终端内容无关。
- 今天的实际状态：**两项协议都不支持，且不支持是静默的**
  （无提示、无占位、无报错）。

### 0.3 会话管理与跨平台安装包：快照只记标题与目录，包只有 .deb

- 会话快照（`src/io/preferences/session_state.rs:12-21`）只有三个字段：
  `title`、`working_directory: Option<String>`、
  `custom_title: Option<String>`。注释写明设计意图：
  「只记标题与工作目录，不记输出；记输出既占体积，也会让『恢复』
  变成伪造历史」。恢复时（`src/app/events.rs:78-98`）按快照重建会话、
  恢复标题与显式名、目录不存在则不恢复（`is_restorable`）。
  **不恢复**：终端输出、scrollback、分屏布局（`PaneTree` 不进快照）、
  窗口位置与大小、搜索/命令记录、会话分组或命名工作区。
  已有的会话能力：标签、P3 分屏（`src/app/pane.rs` 二叉树）、
  标签拖动排序与重命名、关闭确认（P3/P4）。
- 打包只有 Debian 一条路：`scripts/build_deb.py`（229 行）+
  `packaging/`（`io.github.fzf.Siskin.desktop`、`postinst`、`postrm`）。
  P4 已验证：`siskin_0.2.0_amd64.deb`（9,040,694 字节），
  `dpkg-deb --info/--contents`、解包校验、真实 `dpkg -i` 安装与
  `dpkg -r`/`--purge` 卸载干净（见 `docs/compat-matrix.md` 打包验证节）。
  全仓 grep `macos|darwin|windows|flatpak|snap|rpm|homebrew|winget|msi|
  appimage`：除本段评估文字外**零命中**——没有第二平台的任何代码、
  配置或脚本，CI 也只有结构检查与测试（`.agents/loop.yaml`，
  未展开核对不影响结论：仓内无跨平台产物）。
- 今天的实际状态：**会话恢复是「标题 + 目录」的最小快照；
  安装包是「Ubuntu/Debian amd64 .deb」单一路。**

---

## 1. 内置 SSH、密钥管理与连接复用

### 用户场景

- 谁：需要在远程服务器上工作的开发者、运维（多台机器、跳板机）。
  什么情况下：每天 `ssh` 上去改代码、看日志、跑服务；希望标签页
  本身就是一台远程主机，断线重连、记住主机列表、不用来回输口令。
- Siskin 定位下是否必需：**不是**。Siskin 的定位是「安静轻量的
  Linux 终端」，今天用户已经能在终端里直接跑系统 `ssh`（见 §0.1），
  登录、认证、配置（`~/.ssh/config`、ProxyJump、 agent 转发）全部
  复用系统 OpenSSH——这是经过几十年打磨的实现。内置 SSH 解决的
  不是「能不能连」，而是「连得方不方便、管不管得住」。

### 实现路径（两条路线对比）

| 路线 | 做法 | 代价与取舍 |
| --- | --- | --- |
| A. 外调系统 `ssh` 二进制（现状的延伸） | 会话类型仍是本地 PTY，派生的命令从 shell 换成 `ssh user@host`；主机列表只存「要连哪台」，由 `ssh` 读 `~/.ssh/config`；Siskin 不碰密钥、不做协议 | 代码量最小（估计数百行：会话类型 + 主机列表 UI + 断线状态）；零新依赖；认证、 agent、跳板机全部继承系统能力。但做不到连接复用（每次新标签都是一次新 `ssh`）、做不到不断线重连、做不到远程目录感知 |
| B. 内置 SSH 协议栈 | 引入 `russh`（纯 Rust，带 `tokio` 异步运行时）或 `ssh2`（绑定系统 `libssh2`），自己走密钥协商、会话通道、端口转发；密钥管理接系统钥匙环或自存加密文件 | 协议、认证、主机密钥校验、断线重连、复用（multiplex）全是新代码，估计数千行起步；还要长期跟进协议与 CVE；但能做到标签即连接、复用通道、远程感知 |

路线 A 是路线 B 的子集：先做 A 并不堵死 B，但 A 的价值上限很低
（本质上只是给 `ssh host` 加了个图形启动器）。

### 依赖范围

- 路线 A：**零新增直接依赖**（只用 `std::process` 派生已有能力）。
- 路线 B：`russh` 会拖入 `tokio`、加密原语（`ring` / `ssh-key` 等）
  一整条异步 + 密码学依赖树，二进制体积与编译时间明显上升
  （量级估计：数 MB 级增长，以实际 `Cargo.lock` 为准，不编数字）；
  `ssh2` 则把系统 `libssh2` 变成运行期依赖，打包的 `Depends`
  要加，跨发行版行为不再一致。两种都需要一个密钥存储方案：
  系统钥匙环（Linux 上是 Secret Service / D-Bus，又一个新依赖面）
  或自管加密文件（自己写加密与口令流程）。

### 安全边界

- **主机密钥校验是红线**：内置实现必须在首次连接时向用户展示
  指纹并显式确认、之后严格比对 `known_hosts`，绝不能为图方便
  默认接受；这是 SSH 防中间人的唯一屏障，写错一次就是可被利用的漏洞。
- 密钥材料绝不能以明文落盘、不进日志、不进性能采样、不进崩溃报告；
  私钥口令只在内存里短暂持有，用完清零；`ssh-agent` 转发默认关闭，
  开启时要明确告诉用户「远程主机能用你的 agent 签名」。
- 口令认证要走系统安全输入、不回显，失败次数与锁定策略要与
  OpenSSH 行为一致，不自己发明退避。
- 路线 A 没有这些问题——因为它把安全责任完全留给 OpenSSH。

### 可维护成本

- 路线 A：**小**。与现有架构无冲突（会话仍是 PTY + shell），
  升级负担约等于零。
- 路线 B：**大**。新增一个常驻协议子系统：密钥格式演进
  （Ed25519、FIDO2、证书认证都在变）、算法弃用（旧 KEX/签名算法
  下线）、CVE 跟进、与 PTY 读线程的交互（远程断线时 `ChildExit`
  语义不再是本地进程退出）。一旦接手，就要长期有人盯安全公告；
  而 Siskin 目前连 GUI 基线都还没测完，没有这个人力余量。
- 冲突点：现有会话模型假设「子进程退出 = 会话结束」，
  远程会话的「断线 ≠ 进程退出」要重写这条假设（`src/io/pty.rs`
  与 `src/app/events.rs` 的退出处理）。

### 结论：暂缓

- 复评触发条件：大王明确要「远程开发」场景（例如每天在 3 台以上
  服务器之间切换、且对每次重连输口令/等握手感到烦），或有用户
  反馈要求终端原生管理远程会话。触发时**先做路线 A 的最小版**
  （主机列表 + 一键 `ssh` + 断线提示），再评估是否值得走路线 B；
  不直接跳进内置协议栈。

### 同行现状（2026-11-11 核实）

- **WezTerm**：有内置 SSH 客户端与 SSH domains，可连远程 multiplexer，
  标签即远程会话（来源：FreshPorts 的 WezTerm 介绍，
  <https://www.freshports.org/x11/wezterm/>）。
- **Ghostty**：没有内置 SSH 客户端；1.4 新增 `ghostty +ssh`，
  只是系统 `ssh` 的包装，负责把 Ghostty 的 terminfo 装到远程机器、
  转发相关环境变量（来源：OMG! Ubuntu，
  <https://www.omgubuntu.co.uk/2026/10/ghostty-1-4-released/amp>；
  Linuxiac，<https://linuxiac.com/ghostty-1-4-terminal-emulator-cuts-memory-usage-boosts-performance/>）。
- **Otty**：**未查到**——搜索返回的都是名字相近的其他产品
  （OdyTTY、Omnitty 等），没有关于 Otty 本身 SSH 能力的可靠来源，
  不写推测。

---

## 2. Kitty Graphics Protocol 与 Sixel

### 用户场景

- 谁：在终端里用文件管理器、看图、做数据可视化的用户。
  什么情况下：`yazi` / `superfile` 里预览图片、`neovim` 插件贴图、
  `matplotlib` / `gnuplot` 把图表直接画在终端里、远程 `ssh` 上来
  `icat` 看一眼截图。
- Siskin 定位下是否必需：**目前不是必需，但这是三项里最接近
  「用户看得见的缺口」的一项**。文本终端的核心体验不受影响；
  但一旦用户用 `yazi`，没有图像协议就只能看到半块字符画或空白，
  属于「功能不存在」，不是「体验差一点」。

### 实现路径（两条路线对比）

| 路线 | 做法 | 代价与取舍 |
| --- | --- | --- |
| A. 只做 Kitty 图形协议 | 在 `src/io/vt/` 之前（或并行）加 APC 解析：base64 载荷按 id 入图像存储，放置（光标处 / 指定 cell / 相对位置）写进 `Frame`，GPUI 画一层图像（已有 `image` crate 可解 PNG；动画 APNG 另算）；滚动时图像随行移动或清除 | 现代协议、一次到位：真彩色、透明、动画、相对放置；`yazi` 等新工具默认走它。工作量集中在「图像层」这个新概念上：`Frame` 加图像层、滚动/重排/分屏时的图像归属、缓存淘汰 |
| B. 只做 Sixel | DCS `q` 解析 + 六像素解码 + 调色板量化，画到同一图像层 | 老协议、兼容面最广（xterm、foot、WezTerm、Windows Terminal 都支持）；但色彩量化、带宽大、无缩放感知，属于「为兼容旧工具而做的旧实现」，且 Sixel 的滚动语义本身就不清晰 |
| C. 两项都做 | A + B 共用图像层 | 覆盖最全，但解析器与解码器是两套，测试矩阵翻倍 |

建议的顺序是 **A 优先，B 视反馈再定，C 不主动做**：新工具
（`yazi`、`superfile`）都优先探测 Kitty 协议，Sixel 更多是
「老工具的 fallback」。

### 依赖范围

- 解析与放置层：**可零新增依赖**（APC/DCS 状态机、base64 解码、
  图像缓存都能在仓内写；`base64` 如需引入是小而稳的 crate）。
- 解码：已有 `image =0.25.10`（仅 `png` feature，见 §0.2），
  Kitty 协议常用 PNG 直传，现有依赖够用；要支持 JPEG/WebP
  才需给 `image` 开 feature（体积与编译时间小幅上升）。
  Sixel 如走纯 Rust 解码可零依赖；如绑 `libsixel` 则多一个
  系统库依赖，打包要加，不推荐。
- 不需要升级 `alacritty_terminal`：图像层可以建在它外面
  （在 `src/io/vt/` 侧接管 APC/DCS 字节，再合成进 `Frame`），
  但这也意味着绕开后端自研解析，OSC 133 当年就是这么处理的
  （`src/io/pty_read.rs` 自有读线程 + `src/io/osc133.rs` 扫描器）。

### 安全边界

- **图像协议是不可信输入**，与 OSC 52 同级、比 OSC 8 更危险：
  任何程序（包括网页里 `curl` 下来的内容、`cat` 一个恶意文件）
  都能发图。必须设硬上限：单图载荷、解码后像素、同时驻留图像数、
  动画帧数与总内存，超了就丢弃并留一条可见提示，不静默吃掉。
- 解码器是经典攻击面：畸形 PNG、超大尺寸声明（解码炸弹）、
  动画帧爆炸都可能让内存或 CPU 失控；解码前先校验声明尺寸，
  解码结果进有界缓存（可复用 P1 整形缓存的 LRU 思路），
  与 10,000 行历史、512 项整形缓存同一张上限表管理。
- 前车之鉴：Ghostty 1.4 一次修了十个漏洞，其中就包括
  「精心构造的图形命令可导致终端崩溃」
  （来源：Linux und Ich，
  <https://linuxundich.de/en/ghostty-1-4-linux-terminal/>）。
  Siskin 若做图像协议，模糊测试（畸形 APC/DCS 流）要和功能
  同批落地，不能后补。
- 隐私：图像内容不进日志、不进采样、不参与崩溃上报。

### 可维护成本

- **大**。这是三项里对现有架构冲击最大的：`Frame` 要加图像层
  （`src/io/vt/view.rs` 的 `Frame`/`RowView` 结构要改），
  滚动、resize、分屏、搜索高亮、选区复制都要回答「图像怎么办」
  （跟随行滚？清掉？复制时丢掉？）；GPUI 侧要新增图像绘制路径，
  与现有文本 run 合并逻辑并行；动画还要有时钟与失效机制。
  代码量级估计：解析 + 存储 + 绘制 + 测试约一千数百行，
  且后续要跟 Kitty 协议的演进（动画、相对放置是 1.4 才补全的，
  见 Ghostty 的例子）。
- 冲突点：P1 刚把 `Frame` 做成 `Arc` 共享快照，图像像素数据
  （可能数 MB）进 `Arc<Frame>` 会放大每次快照的内存占用，
  必须让图像数据独立于 `Frame` 引用计数（存 id + 共享图像池），
  否则一次全屏图就能把快照成本翻几倍。

### 结论：可考虑（下一轮候选第一位）

- 复评触发条件：有用户反馈「在 Siskin 里用 `yazi` 看不到图片」、
  或大王自己开始在终端里看图/看图表。触发时**只做 Kitty 协议、
  不做 Sixel**，且先定上限表与模糊测试，再写绘制代码。
  Sixel 只有在「有用户明确要 Sixel、且 Kitty 满足不了」时才复评。

### 同行现状（2026-11-11 核实）

- **Ghostty 1.4**：Kitty 图形协议完整支持（含动画与相对放置），
  是除 Kitty 外第一个做到完整的终端（来源：OMG! Ubuntu，
  <https://www.omgubuntu.co.uk/2026/10/ghostty-1-4-released/amp>；
  Linuxiac，
  <https://linuxiac.com/ghostty-1-4-terminal-emulator-cuts-memory-usage-boosts-performance/>）。
- **WezTerm**：支持 Kitty 图形协议与 Sixel（上游把 Sixel 标为实验性），
  另有自家的 iTerm2 兼容图像协议（来源：Termium 终端支持页，
  <https://www.termium.dev/docs/terminals/>；
  FreshPorts，<https://www.freshports.org/x11/wezterm/>）。
- **Alacritty**：原生不支持任何内联图形协议（对照表里写
  `None (Requires patches)`），Sixel 只有打补丁的版本能用，
  且不清图形（来源：ratatui-image 文档，
  <https://docs.rs/crate/ratatui-image/0.8.0>；
  Kitty 对照表，<https://hamradio.my/kitty-gpu-terminal-emulator-guide/>）。
  注：Siskin 与 Alacritty 共用 `alacritty_terminal` 后端，
  所以这个「没有」是现状的直接印证，不是巧合。

---

## 3. 更完整的会话管理与跨平台安装包

这两件事常被并列，但性质完全不同，分开评估：

### 3a. 更完整的会话管理

#### 用户场景

- 谁：同时开很多项目、很多标签的用户。什么情况下：重启终端后
  希望昨天的分屏布局、每个 pane 的目录、滚动历史都回来；
  希望把一组标签存成「工作区」（如「后端调试」「写文档」），
  一键恢复；希望会话能按项目分组、搜索、归档。
- Siskin 定位下是否必需：**部分必需**。重启后只恢复标题与目录
  （§0.3）对轻度用户够了；但对「终端就是工作台」的用户，
  每次重启重建分屏与目录是每天都在付的税。

#### 实现路径（两条路线对比）

| 路线 | 做法 | 代价与取舍 |
| --- | --- | --- |
| A. 补齐恢复（小步） | 快照加分屏布局（`PaneTree` 序列化）与窗口几何；恢复时按布局重建 pane、每叶回对应目录；仍不恢复输出 | 估计数百行；复用现有快照落盘链路（`src/app/prefs_ops.rs:124` 的 `session_snapshots` + 500ms 防抖保存）；风险低。输出仍不恢复——这是有意的设计（见 §0.3 的注释），不是欠账 |
| B. 工作区 / 会话持久化 | 命名工作区、多窗口布局保存、滚动历史落盘（或按需重放）、会话搜索与分组 | 估计数千行；历史落盘要回答体积上限、隐私（终端输出可能含密钥）、跨机器同步；基本等于再造一个 tmux 的会话层 |

#### 依赖范围

- 路线 A：**零新增依赖**（`serde` 已有，快照就是 JSON）。
- 路线 B：仍可零依赖，但存储格式、迁移链（已有 `v1 → v2` 的先例）、
  损坏回退都要跟着长大；若要跨机器同步才会引入新依赖，不建议。

#### 安全边界

- 终端输出**不建议落盘**：输出里可能有口令、token、私钥内容，
  落盘等于把敏感信息写进配置文件；现在「不记输出」的设计
  是对的，不要为「恢复更完整」推翻它。真要恢复历史，
  应只恢复「目录 + 正在跑的命令名」这种元信息。
- 快照文件已有 64 KiB 上限与损坏回退（P4），加字段时要守住
  这条线：布局变大后先算上限，超了就截断而不是放开。

#### 可维护成本

- 路线 A：**小**。与现有架构顺滑（`PaneTree` 已是纯数据，
  `src/app/pane.rs`），升级负担是快照字段的版本兼容，老快照
  缺字段按默认加载（已有 `#[serde(default)]` 的先例）。
- 路线 B：**中到大**。工作区概念会扩散到命令面板、设置页、
  快照迁移与冲突处理，且与 tmux 的能力重叠——用户已经可以在
  Siskin 里跑 tmux，没必要在终端里再造一个。

#### 结论：路线 A 可考虑，路线 B 暂缓

- 复评触发条件：大王或用户反馈「重启后重建分屏太烦」。
  触发时只做路线 A。工作区（路线 B）触发条件是「有人明确
  要在 Siskin 里管理多组项目会话、且不愿用 tmux」——目前没有
  这样的信号。

#### 同行现状（2026-11-11 核实）

- **Ghostty**：会话管理是**有意的缺失**——官方立场是
  「Ghostty 是终端，tmux 与 Zellij 继续干它们的工作」
  （来源：Botmonster，
  <https://botmonster.com/coding/ghostty-terminal-emulator-libghostty/>）。
- **WezTerm**：内置 multiplexer（标签、pane、窗口）并可连本地
  与远程 multiplexer（来源：FreshPorts，
  <https://www.freshports.org/x11/wezterm/>）——属于重功能路线，
  与 Siskin 的轻量定位正好是两个端点。
- **Otty**：**未查到**（同 §1，搜索只返回名字相近的其他产品，
  无可靠来源，不写推测）。

### 3b. 跨平台安装包（macOS / Windows）

#### 用户场景

- 谁：想在 macOS 或 Windows 上也用 Siskin 的用户。
  什么情况下：主力机是 Mac/Windows，或跨平台工作。
- Siskin 定位下是否必需：**不是**。Siskin 从第一天起就是
  「Linux 终端」：`Cargo.toml` 的描述就是
  `A focused Linux terminal built with pulse-ui and GPUI`，
  打包、菜单（D-Bus）、输入法、桌面入口全部按 Linux 桌面设计。
  跨平台不是「多打两个包」，而是「换一个产品定位」。

#### 实现路径（两条路线对比）

| 路线 | 做法 | 代价与取舍 |
| --- | --- | --- |
| A. 只做 macOS | GPUI 本身支持 macOS（Zed 的主平台），代码改动相对小；但要补 macOS 打包（`.dmg` / Homebrew）、签名与 notarization、菜单/快捷键的 macOS 习惯（`Cmd` 系）、输入法与触控板 | 估计中等工作量，但从此每个发版都要在两台机器上验证，CI 从单平台变双平台 |
| B. macOS + Windows | 再加 Windows 打包（`.msi` / winget）、Windows Terminal 式 PTY（ConPTY，与现有 Unix PTY 读线程 `src/io/pty_read.rs` 是两套）、签名（SmartScreen）、高 DPI 与字体回退的另一套行为 | 工作量翻倍以上；PTY 层要抽象两套后端，`src/io/pty.rs` 与 `src/io/pty_read.rs` 的 Unix 假设要全部重审。约等于维护两个终端 |

#### 依赖范围

- 代码依赖可零新增（GPUI 已跨平台），但**发布依赖**是新增的：
  Apple Developer 签名证书与 notarization 流程、Windows 代码
  签名证书、双平台 CI runner、双平台测试矩阵。证书是按年付费、
  按人持有的东西，不是写进 `Cargo.toml` 就完事的。

#### 安全边界

- macOS：未签名/未 notarize 的应用会被 Gatekeeper 拦截，
  用户要手动放行——对「安静轻量」的定位是负分；签名密钥的
  保管与轮换是持续责任，泄露等于要吊销重发。
- Windows：SmartScreen 对无信誉签名同样拦截；ConPTY 的
  进程模型与 Unix PTY 不同，子进程退出、信号、编码
  （UTF-16 与 UTF-8 的边界）都是新的异常面。
- 两平台都要回答「自动更新怎么做」：Linux 有系统包管理器，
  macOS/Windows 各需一套更新通道，否则用户永远停在旧版
  （旧版 = 已知漏洞不修）。

#### 可维护成本

- **大**。不是一次性工作，是每个发版、每次依赖升级、每次
  平台行为变化都要付的税；且 Siskin 的人力现实是「一个作者 +
  一个 agent」，连 Linux 下的 GUI 基线（S1–S7）都还没测完。
  冲突点：D-Bus 全局菜单、X11/Wayland 假设、`.deb` 打包脚本
  全是 Linux 专属，跨平台等于在代码里到处加 `cfg(target_os)`，
  结构门禁（`scripts/check_structure.py`）与测试矩阵都要跟着改。

#### 结论：不建议

- 复评触发条件：**定位本身改变**，即大王明确说「Siskin 不再
  只是 Linux 终端，要做跨平台产品」。在此之前，macOS/Windows
  用户已有 Ghostty、WezTerm、Windows Terminal 等成熟选择，
  Siskin 去挤这个赛道没有胜算；把精力留在 Linux 体验上更划算。

---

## 4. 总表

| # | 能力 | 结论 | 触发复评的条件 | 粗略成本 |
| --- | --- | --- | --- | --- |
| 1 | 内置 SSH、密钥管理、连接复用 | 暂缓（先做外调系统 `ssh` 的最小版） | 大王明确要远程开发场景（多机切换、嫌重连麻烦），或用户反馈要原生远程会话管理 | 路线 A 小；路线 B 大 |
| 2 | Kitty Graphics Protocol 与 Sixel | 可考虑（下一轮候选第一位；只做 Kitty，不做 Sixel） | 有用户反馈在 Siskin 里用 `yazi` 看不到图片，或大王开始在终端里看图/看图表 | 大 |
| 3a | 更完整的会话管理（恢复分屏布局等） | 可考虑（只做补齐恢复，不做工作区） | 大王或用户反馈「重启后重建分屏太烦」 | 小 |
| 3b | 跨平台安装包（macOS / Windows） | 不建议 | 定位本身改变：大王明确说 Siskin 要做跨平台产品 | 大 |

## 5. 给大王的建议

**下一轮先候选 Kitty 图形协议（只做 Kitty，不做 Sixel）；明确不做
macOS / Windows 安装包；SSH 保持现状（在终端里跑系统 `ssh`），
等远程开发场景明确了再做「主机列表 + 一键 ssh」的最小版。**
会话恢复可以顺手补「分屏布局」这一小步，但别碰工作区——
那是 tmux 的活。
