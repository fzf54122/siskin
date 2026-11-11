<!--
@Time    : 2026/11/5 20:02
@Author  : fzf
@FileName: CHANGELOG.md
@Software: Siskin
-->

# 变更记录

## 未发布

### 新增（AI 终端 UI 重设计，桌面效果待大王在 Linux 桌面上确认）

- 磨砂玻璃（见 `docs/glass.md`，桌面效果待大王在 Linux 桌面上确认）：
  命令面板、搜索条、Agent 侧栏、检查器、设置导航栏与弹窗用半透明底
  + 细边框 + 分层透度做玻璃感，终端主区保持实底。先核实过 GPUI 与
  pulse-ui 源码：GPUI 没有逐元素 backdrop blur，只有窗口级
  `WindowBackgroundAppearance::Blurred`，Wayland 下走 KDE 的
  `org_kde_kwin_blur` 协议（仅 KWin 实现），X11 下等价于普通透明，
  pulse-ui 没有相关接口——所以真模糊只在 KDE Plasma Wayland 上叠加，
  其余平台是半透明近似，未硬编。开关在设置页外观（另有 Ctrl+Shift+G
  与命令面板「玻璃效果开/关」「玻璃透度」两条，面板共 39 条）；透度
  三档 0.85 / 0.90 / 0.95，默认开启 0.85，下限是逐套验算后能保住
  WCAG 4.5:1 的最低值（0.75 时 Paper 浮层 4.34:1、Moss 侧栏 4.30:1
  均跌破），三档 × 六套预设有对比度断言。顶栏 tab 条因 faint 目录
  文字保不住对比度而保持实色。开关与透度随设置落盘，旧配置默认开启。

- 配色修整与两套新主题（见 `docs/themes.md`，桌面效果待大王在
  Linux 桌面上确认）：命令面板（Ctrl+K）搜「主题」可直接点选六套
  预设——原有 Paper / Graphite / Moss / Dusk 加新加的 Catppuccin
  Mocha、Tokyo Night；点选固定生效并随设置落盘，换模式即解除；
  Ctrl+Shift+T 仍按模式循环。修整：Paper 强调色原来与正文同色，
  光标分不出来，改成深 slate 蓝 `#2f5b8f`；Moss 的弱化文字
  （2.8:1）与系统消息色（3.9:1）、Dusk 的弱化文字（3.6:1）均未达
  WCAG 4.5:1，已按同色系加深修正；Graphite 全部达标未动。新主题
  色值已对官方色板核对（Catppuccin Mocha 取自上游 palette 数据，
  Tokyo Night 取自 `tokyonight.nvim` Night 变体并与 VS Code 终端色表
  交叉核对；仅 Tokyo Night 弱化色因官方 dark5 只有 4.1:1 而提亮一档
  到 `#7f8bb3`）。对比度断言现逐套覆盖六套预设，另加光标可见性断言，
  阈值未放宽。

- 命令面板升级（Ctrl+K）：模糊搜索（子序列匹配、命中字符强调色加粗
  高亮），覆盖除打开面板自身外的全部 `Action`（补了退出、回到终端、
  放大/缩小字号分开、向上/向下翻页、滚到最旧输出、四个默认 Shell
  条目、Agent 侧栏，共 37 条）；空查询时最近 5 条操作排在最前，有查询时最近操作
  加权置前。模糊打分、排序、动作全覆盖均有单测。
- Agent 侧栏（`Ctrl+Shift+A` 或 tab 栏列表按钮，默认收起）：每个会话 /
  pane 一条，显示当前命令（OSC 133 `C` 到达时从网格读入的文本）、
  状态（运行中 / 完成 / 失败 / 空闲）、耗时与退出码；失败条目带红色
  「失败」徽章，收起时 tab 栏按钮右上角给红点；点条目切到该会话所在
  标签并聚焦对应 pane。无标记的 shell 回退到「空闲」，不猜它在跑什么。
  只呈现已有会话的状态，不做 agent 运行时、不做文件侧栏、不做实时预览。
  条目 250ms 节流重算，会话新建 / 退出时同步更新；见
  `docs/agent-sidebar.md`。状态推导、耗时格式、开合、点击聚焦、
  失败徽章均有单测；GUI 层面云端 VM 无显示器/Vulkan，桌面效果待大王
  在 Linux 桌面上确认。
- 行内自动补全（ghost text）：输入时按本会话已执行命令联想，
  完整命令的剩余后缀以弱化色画在光标之后，只画、不写进 PTY；
  `Tab` 接受整条，`→` 接受一个词，`Esc` 取消；换行、新输出、
  失去焦点时消失。联想只来自本会话输入记录 + OSC 133 命令记录
  （`C` 标记到达时从网格读入，最近 200 条，去重），不读 shell
  历史文件、不联网、不调外部 AI。只在 shell 发了 OSC 133
  `A`/`B`/`C` 时才工作，见 `docs/shell-integration.md`；
  多行输入、行中光标、宽字符都有边界处理，单测 + 真实 PTY 测试覆盖。

### 修复（手感，桌面验收待大王在 Linux 桌面上确认）

- Ctrl+D 之后不再留死窗格：子进程退出且无需关闭确认时直接关闭
  该标签/窗格（分屏只关退出会话所在的叶，最后标签走已有窗口关闭
  逻辑）；有运行中任务需确认时保留现有确认流程。空提示符 Ctrl+D
  真实 PTY 测试覆盖；非空提示符 Ctrl+D 仍是 shell 的 EOF 语义，
  应用层不多管。
- 滚进历史时不再画底行的幻影光标：`display_offset > 0` 时可视区不画
  光标（真实光标行在可视区之下），回到底部即恢复；输入法锚点同步
  不留在幻影位置。离线帧测试覆盖。
- 网格右侧新增细滚动条：位置与长度反映
  `display_offset / history_size`，点击轨道与拖动 thumb 均可跳转，
  无历史时不画；细条、低对比（`faint`），与“回到底部”按钮共存，
  不改滚轮行为。thumb 换算、点击/拖动映射单测覆盖；跳转 plumbing
  真实 PTY 测试覆盖。

### 新增

- P5 评估记录（`docs/p5-evaluation.md`，只评估、未实现、零代码改动）：
  仓内现状盘点——SSH 零代码零依赖（只靠系统 `ssh` 子程序）、
  Kitty/Sixel 在 `alacritty_terminal 0.26.0` + `vte 0.15.0` 里被静默丢弃、
  `Frame`/`CellView` 无图像层、会话快照只记标题与目录、
  安装包只有 Ubuntu/Debian `.deb` 单一路。结论：SSH 暂缓
  （远程开发场景明确时先做外调系统 `ssh` 的最小版）；Kitty 图形协议
  可考虑为下一轮候选第一位（只做 Kitty，不做 Sixel）；会话管理只补
  分屏布局恢复可考虑（不做工作区）；macOS/Windows 安装包明确不建议。
  外部事实 2026-11-11 核实（Ghostty 1.4、WezTerm、Alacritty 附来源；
  Otty 未查到，写明未查到、不写推测）。plan.md §9 已记进展注。
- P4 兼容性矩阵与异常恢复（`docs/compat-matrix.md`，30 个新测试）：
  OSC 52 写接受、读拒绝、非法 base64 丢弃（`OnlyCopy` 策略，离线与真实
  PTY 各一测）；OSC 8 两段链接各归其主，白名单外的 `javascript:` 点击
  被拦截并提示；鼠标 SGR/X10 报告离线编码与真实 PTY 双覆盖（`cat`
  收到的字节精确比对，Shift 留给选区）；256 色/true color 断言帧颜色值；
  光标形状 DECSCUSR 1–6；宽字符补混合行光标列对齐；`TERM`/`COLORTERM`/
  `TERM_PROGRAM`/`TERM_PROGRAM_VERSION` 真实 PTY 断言；resize 后 `stty size`
  与网格行数、输出继续流动；Zsh 5.9 与 tmux 3.4 真实 PTY 验证。异常恢复：
  shell 不存在解析失败不 panic、PTY 派生返回 Err；`kill -9 $$` 报
  `ChildExit(None)` 不谎报 0，`exit 137` 原样报 `Some(137)`；无 D-Bus 时
  上报全局菜单不可用、不崩溃；配置损坏整份走默认回退且不覆盖原文件，
  单条坏快照只丢那一条（宽容反序列化）。内存上限已审计成表：
  历史 10,000 行、整形缓存 512 项/4 MiB、搜索 1000 条、命令记录 100 条、
  配置 64 KiB；剪贴板写队列新加 64 条上限；`seq 1M` 实测历史恰好
  10,000 行、RSS 增长单跑 17.3 MiB、全套并行 31.8 MiB，上界 64 MiB。
  X11/Wayland 下的窗口呈现、字体回退、输入法、Fish/vim/htop 仍未验证，
  见矩阵文档。

### 修复

- P4 回归发现：X10 纯点击模式的松开曾误发一条形似按下的报告
  （注释写明「只上报按下」，空分支实际仍发出），现返回空、SGR
  模式不受影响。`file://` 链接注释曾写「只在路径存在时才算有效」，
  代码实际不检查存在性、直接交给系统 opener，注释已改成与代码一致，
  点击行为未改。
- P4 打包验证修了 `scripts/build_deb.py` 的两处环境问题：本机 umask
  `0007` 且各级目录是 `2770`（setgid）时，`DEBIAN` 目录会继承出
  `2770` 而被 `dpkg-deb` 拒收，现显式 `chmod(0o755)`；打包前把 stage
  内所有目录统一收成 `0755`。`siskin_0.2.0_amd64.deb` 已验证：
  `dpkg-deb --info/--contents`、解包校验、真实 `dpkg -i`（只缺声明依赖
  `librsvg2-common`，用 `--force-depends`）与 `dpkg -r`/`--purge` 卸载干净；
  `desktop-file-validate` 与 `lintian` 本机未装，记未验证。

- P3 标签拖拽排序与显式重命名：按住标签拖动即可在 tab 条内
  换位（移过别的标签时当场换位，被拖标签半透明，松开结束；
  不做拖出窗口成新窗口、不做跨窗口）。双击标签进内联编辑，
  命令面板也有「重命名当前标签」；回车/Tab 提交，Esc 取消，
  清空提交清除显式名，失焦（点别的标签、点进终端）即提交。
  显式名优先于目录名回退，哪怕改成「终端」也照显；清除后恢复
  目录回退。改名与顺序都进会话快照（`custom_title` 字段，老快照
  无此字段正常加载），顺序即向量顺序，不另存字段。分屏不受影响：
  移动只改向量顺序不改会话 id，焦点 pane 跟随不受影响。
- P3 OSC 133 Shell Integration：解析 `133;A/B/C/D` 标记，每会话保留最
  近 100 条命令（起始行、退出码、耗时），退出码只认 `D` 标记参数。
  命令间跳转 `Ctrl+Shift+Up` / `Ctrl+Shift+Down`（命令面板也可查），
  目标命令滚到可见区顶部；失败命令只在记录里标失败，不做新面板。
  上游解析层丢弃 133 标记，因此 PTY 读线程改走自有实现
  （`src/io/pty_read.rs`，读/写/退出/`drain_on_exit`/同步超时行为与
  上游 `EventLoop` 对齐），扫描只观测、不剥离，行号锚定用单调稳定行。
  Shell 配置见 `docs/shell-integration.md`：Bash 与 Zsh 片段已在真实
  交互式 shell 上验证（`false` 正确报 `D;1`）；Bash 的 `C` 靠 `DEBUG`
  trap 实现，会多触发一次，耗时不准；Fish 本机未装，片段标为未验证。
  无标记的 shell 正常工作，命令记录为空是合法状态。
- P3 历史搜索：覆盖完整 scrollback（10,000 行），不再只搜当前可见帧。
  走 `alacritty_terminal` 的 `RegexSearch` / `Term::search_next` 直接扫网格，
  不拼整份文本、不新增 `regex` 依赖；匹配上限 1000 条。字面模式转义正则
  元字符；大小写选项（`Alt+C`，搜索条 `Aa` 按钮）与正则模式（`Alt+R`，
  搜索条 `.*` 按钮）用 `(?i)` / `(?-i)` 前缀绕过内置智能大小写；无效正则
  在搜索条 label 显示「正则错误：…」，不崩溃、不清空查询词与开关。
  步进（Enter / Shift+Enter / Up / Down）把目标历史行滚进可见区再高亮，
  两端循环环绕；新 PTY 输出到达时自动重算，但查询词、选项与帧快照 `Arc`
  指针都没变时直接复用，不重复扫历史；滚动只同步缓存帧指针。10k 行单次
  全量搜索在测试里 < 2 秒。
- P3 分屏：单窗口内二叉布局（`src/app/pane.rs` 的 `PaneTree`，
  叶按会话 id 引用，分屏叶不进标签栏），一个标签一棵树。键位：
  `Ctrl+Shift+D` 向下分屏、`Ctrl+Shift+R` 向右分屏、
  `Ctrl+Shift+J/K` 上/下一个 pane、`Ctrl+Shift+Q` 关闭当前 pane
  （只剩一叶时等同于关标签）；分隔线可拖动调比例（0.1..=0.9）。
  焦点 pane 驱动输入、滚动、搜索与窗口标题；窗口 resize 时每叶
  按自己的矩形算行列并通知 PTY。关闭 pane 有运行任务时走与关会话
  同一套确认框；关标签时连带关掉该标签的全部叶会话。
- P1 渲染热路径：`Session.frame` 改为 `Arc<Frame>` 共享快照，
  `frame_snapshot()` 让渲染与搜索拿同一份 `Arc`，不再每次深拷贝
  整帧；`refresh()` 只在脏时替换 `Arc`，干净会话跳过构帧，
  旧快照仍完整可读，不会读到半更新帧。选区矩形合并补宽字符
  spacer 处理，不留缺口。Unicode 测试覆盖 CJK 双宽、Emoji、
  组合字符、Nerd Font 图标与混合宽度对齐。整形缓存键变化
  （前景色、字号、字体族）未命中测试。
- P2 PTY 更新合并：事件分可合并（`Wakeup`/`Bell` 只标脏）与
  不可丢、不可乱序（标题、剪贴板、PTY 回写、退出）两类；
  轮询间隔可用 `SISKIN_POLL_MS` 覆盖（2..=100ms，默认 16ms）。
  实测 `seq 1 1000000` 在 16ms 与 2ms 下均无丢字节，2ms 无
  吞吐收益且轮次 6 倍，默认保持 16ms。无界通道评估后保留，
  理由见 `src/app/events.rs` 注释。
- P0 性能采样：`--perf` / `SISKIN_PERF=1` / 命令面板「性能采样开/关」
  三种开启方式，右上角 overlay 显示 FPS、P50/P95、整形、构帧、
  每轮 PTY 事件、刷新会话与绘制任务数、整形缓存命中率。
  实现在 `src/domain/perf.rs`（纯数据层，可单测），默认关闭，
  关闭时不给正式渲染加固定开销；只记数字，不记终端内容、
  命令行或剪贴板。固定场景与基线记录模板见 `docs/perf-baseline.md`。
  GUI 基线数字尚未在真实桌面会话里测出，表中明确标「未测」。
- 文本整形有界缓存：`ui/grid/` 每次 `shape_line` 先查缓存，命中即返回。
  缓存键覆盖文本、字体族、字重、斜体、下划线、字号、scale 与前景色，
  使用 LRU，条目数与估算字节数双上限（512 条 / 4 MiB），
  滚动长历史不会无限增长。真实 GPUI 整形测试验证同一组文本第二轮全命中。
  新增 `ui/grid/cache.rs`。
- 网格绘制拆分：`ui/grid/paint.rs` 承接一行的背景、文本与链接底线，
  `ui/grid/element.rs` 承接 `Element` 生命周期，`mod.rs` 只留构帧逻辑。

### 变更

- 终端鼠标移动仅在拖选端点换格或鼠标报告实际发出时通知 UI，
  同一格内抖动不再白跑一帧；`drag_to()` 返回是否变化，鼠标上报
  的发送路径不受影响。
- 选区背景按连续段合并：一行相邻的选中格子合成一个矩形，
  80 列全选从 80 个绘制任务降到 1 个；断开处仍分段，不会糊成一片。
- PTY 事件轮次只刷新有输出的会话：`Session` 新增脏帧标记，
  `drain_events` 每轮只对脏会话构帧，空闲会话不再白锁 PTY、白拷一帧网格。
- 命令行程序名按 `PATH` 解析，且接受 `/bin/sh` 这类指向可执行文件的符号链接；
  以前 `siskin -e sh` 会按当前目录查找并误报「找不到可执行文件」。

### 修复

- 搜索高亮原来只在 `grid_layer` 的 `focused` 为真时画，而搜索条打开时它
  恒为假，导致边打字边看不到匹配；现在高亮只按焦点 pane 判定，搜索时照常显示。
- 鼠标点击换算原来按网格原点、没补字体留白，点击位置会偏约 3 列；
  现在按卡片原点 + 留白换算，与光标绘制和列数计算用同一口径。
- `ui/grid/mod.rs` 超过 400 行结构门禁，随本次改动拆出 `paint` / `element`
  两个文件。

## 0.2.0 — 2026-11-05

从界面演示变成可日常使用的终端。

### 新增

- 真实 PTY：会话跑在独立伪终端里，支持 `vim`、`htop`、`tmux`、`ssh`。
  派生、读线程、resize（`TIOCSWINSZ`）与子进程回收都在 `io/pty.rs`。
- VT 解析：用 `alacritty_terminal` 0.26 的 `Term` + `vte::Processor`，
  统一封装在 `io/vt/`，其它模块只看见 `Frame` / `CellView`。
- 自定义 `gpui::Element` 网格渲染：按行把 cell 合并成同色 run，一次整形整行；
  只整形可视行；支持粗体、斜体、下划线、反显、宽字符、组合字符与四种光标形状。
- ANSI 颜色：16 色表、6×6×6 色立方、24 级灰阶、truecolor。
- 按键编码：方向键按 `DECCKM` 在 CSI / SS3 间切换，`Ctrl+字母` 写控制码，
  `Alt+键` 加 `ESC` 前缀，带修饰键的功能键走修饰码编码。
- 输入法：实现 `EntityInputHandler`，预编辑串不写入 PTY，候选框跟随光标。
- 选区与剪贴板：拖选、PRIMARY、`Ctrl+Shift+C/V`、`Shift+Insert`、
  bracketed paste 自动包边界，多行粘贴统一成 `\r`。
- 鼠标上报：应用开启鼠标模式时转成 SGR / X10 序列，`Shift` 强制本地选区。
- 命令行：`-d/--working-directory`、`-e <命令>`、`-V`、`-h`。
  `-e` 场景子进程退出即关窗。
- 设置页 Shell 分区改用真实可执行文件路径，显示 `$SHELL` 默认值。

### 变更

- 所有终端动作改为 `Ctrl+Shift+*`，未绑定的组合原样交给 shell。
- 会话仓储按 id 管理，最后一个会话「关闭」等于重启一个新 shell。
- 侧栏显示真实状态（启动中 / 运行中 / 已退出 / 启动失败），
  右侧会话信息显示真实网格尺寸、滚动偏移与子进程 cwd。
- 新图标：白色圆角底 + `>_` 提示符 + 黄色小鸟，成品位图组在 `assets/icons/`。
- 新增 Debian 打包：`packaging/`（desktop、postinst、postrm）与
  `scripts/build_deb.py`，覆盖 16–512px 图标与 `@2` 位图。
- 新增 `scripts/check_structure.py` 结构门禁，并接入 CI 与 `.agents/loop.yaml`。
- 搜索滚动区：`Ctrl+Shift+F`，匹配高亮与当前项区分，`Enter` 循环跳转。
- `Ctrl+Click` 打开链接，含 `http(s)://` 与 `path:line:col`；协议白名单之外不打开。
- 单实例：第二个 `siskin` 转发参数后退出，复用已有窗口。
- 会话恢复：记标题与 cwd，不记输出；目录不存在时按默认目录启动。
- 配置迁移链 `v1 → v2`，版本比当前新时拒绝读取而不是降级覆盖。
- 设置保存改为 500ms 防抖，退出前强制落盘。
- 设置页显示「未能保存设置」，不再只有 stderr。
- 新增 `docs/manual-checklist.md` 发版检查清单与 `build.rs` 资源依赖声明。
- 「清屏」恢复为真实实现：向 PTY 发 `ED 2/3` 序列，并在命令面板补条目。
- 窗口标题跟随 OSC 0/2，会话名不受影响（`vim`、`ssh` 不会改标签）。
- 会话进程退出后，`Enter` 关闭该会话；只剩一个时等于重启。
- 启动失败与链接打开失败的提示显示在标题栏，不再只写 stderr。

### 移除

- `shell.rs` 模拟命令引擎、`fs.rs` 只读文件树、`terminal.rs` 软换行模型、
  `line.rs` 语义文本片段。
- `ShellKind::prompt_marker` / `startup`：提示符由 shell 自己输出。
- 「当前为界面预览」文案与标题栏的 `simulated` 字样。

### 修复

- `Ctrl+R`、`Ctrl+P` 等按键曾被按键链路误吞，现在只在命中终端快捷键时消费。
- 窗口 resize 后程序收不到新尺寸的问题：先算行列再调 `TIOCSWINSZ`，避免回环。

## 0.1.0 — 2026-11-04

界面演示基线。可回退标签 `v0.1.0-demo`：主题系统、分区设置页、
版本化配置（原子写入、损坏不覆盖）、D-Bus 全局菜单、命令面板。
