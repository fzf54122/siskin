<!--
@Time    : 2026/11/11
@Author  : Lux
@FileName: compat-matrix.md
@Software: Siskin
-->

# Siskin 兼容性矩阵（P4）

> 状态只有三种：**已验证**（附证据）、**未验证**（没跑过，不宣称）、
> **不支持**（明确不做）。没有「推测」这一栏。

## 如何读这张表

- **X11 / Wayland 两列**：只对「窗口里看得见、摸得着的行为」有意义。
  本 VM 没有显示器，这类行为两列都只能填未验证。
- **PTY / 解析层**（`OfflineScreen`、真实 PTY、脚本）：不依赖显示
  服务器，X11 与 Wayland 下是同一条代码路径。这类行在两列填
  「已验证\*」，星号统一表示「在 PTY/解析层验证，与显示服务器
  无关；窗口内的最终呈现仍未验证」。
- **证据**：测试名是 `cargo test` 里的完整路径，或可直接跑的命令。
  没跑过的项目不写证据，只写未验证。

## 验证环境（2026-11-11）

| 项目 | 值 |
| --- | --- |
| 发行版 | Ubuntu 24.04.5 LTS |
| 机器 | Muse 云端 VM，1.5 核，无显示器、无 Vulkan |
| siskin | 0.2.0（`Cargo.toml`） |
| Bash | 5.2.21 |
| Zsh | 5.9 |
| tmux | 3.4 |
| Fish / vim / htop | 未安装 |
| `desktop-file-validate` / `lintian` | 未安装 |
| 测试总数 | 269（P4 前 239，P4 新增 30） |

P4 新增测试：`src/tests/compat.rs`（17）、`src/tests/compat_pty.rs`（9）、
`src/tests/compat_gui.rs`（3）。门禁：`cargo test --all-targets --locked`
269 通过 / 0 失败（2026-11-11）。

## 终端兼容性矩阵

### Shell Integration 与剪贴板、链接

| 用例 | X11 | Wayland | 证据 |
| --- | --- | --- | --- |
| OSC 133 标记解析与命令状态、退出码 | 已验证\* | 已验证\* | P3：`tests::pty_osc133` 全组 + `docs/shell-integration.md`；Bash 5.2.21 与 Zsh 5.9 在真实交互式 shell 上验证（`false` 正确报 `D;1`） |
| OSC 133（Fish） | 未验证 | 未验证 | Fish 本机未装；`docs/shell-integration.md` 里的 Fish 片段明确标未验证 |
| OSC 52 写剪贴板被接受 | 已验证\* | 已验证\* | `tests::compat::osc52_write_is_accepted_as_clipboard_store`（`OfflineScreen`）与 `tests::compat_pty::osc52_write_reaches_the_app_through_a_real_pty`（真实 PTY，子程序 `printf` 输出序列） |
| OSC 52 读请求被拒绝、不回写 | 已验证\* | 已验证\* | `tests::compat::osc52_read_query_is_denied_without_a_response`、`tests::compat_pty::osc52_read_query_gets_no_answer_through_a_real_pty`。策略来自 `alacritty_terminal` 的 `Osc52::OnlyCopy`（`src/io/pty.rs` 显式设置；`OfflineScreen` 用同一默认值）与 `VtListener` 只转 `ClipboardStore` |
| OSC 52 非法 base64 被静默丢弃 | 已验证\* | 已验证\* | `tests::compat::osc52_invalid_base64_produces_no_event` |
| OSC 8 链接开始/结束、两段链接各归其主 | 已验证\* | 已验证\* | `tests::compat::osc8_two_links_keep_their_own_uris_and_end_resets`；起始已有 `io::vt::tests::osc8_hyperlinks_reach_the_frame` |
| OSC 8 点击位置与白名单外的 scheme 被拦截 | 已验证 | 已验证 | `tests::compat_gui::osc8_non_whitelisted_scheme_is_not_clickable`：真实 PTY 里 `printf` 输出 `javascript:alert(1)` 链接，`open_link_at` 返回 false，状态栏提示「已拦截不可信链接协议」。白名单是 `http://`、`https://`、`file://`（`src/io/links.rs`、`src/app/terminal_ops.rs::open_uri`）；`javascript:`/`data:`/`ftp:` 不产生可点目标（`io::links::tests::other_schemes_are_rejected`） |
| OSC 8 点击 `http(s)` 链接能打开 | 未验证 | 未验证 | 允许方向会真的调 `xdg-open`，自动化里不触发；只在代码层确认它走与拦截同一条白名单分支 |
| `file://` 链接的存在性检查 | 不支持 | 不支持 | 注释曾写「只在路径存在时才算有效」，但代码实际不检查存在性、直接交给系统 opener——P4 已把注释改成与代码一致。新行为是有意的：不改点击语义，只修文档 |

### 鼠标协议

| 用例 | X11 | Wayland | 证据 |
| --- | --- | --- | --- |
| 模式位翻译（1000/1002/1006） | 已验证\* | 已验证\* | `tests::compat::mouse_modes_translate_from_escape_sequences` |
| SGR 报告按下/松开（`M`/`m`，松开编码 3） | 已验证\* | 已验证\* | `tests::compat::mouse_report_encodes_sgr_press_and_release`（离线）与 `tests::compat_gui::sgr_mouse_report_reaches_the_program`（真实 PTY，`cat` 收到的字节精确等于 `\x1b[<0;5;3M`） |
| X10 报告按下（坐标+32） | 已验证\* | 已验证\* | `tests::compat::mouse_report_x10_release_is_suppressed_in_click_only_mode`（离线）与 `tests::compat_gui::x10_mouse_report_reaches_the_program`（真实 PTY，`cat` 收到的字节精确等于 `ESC [ M SP % #`） |
| X10 纯点击模式不报松开 | 已验证\* | 已验证\* | 同上。P4 回归发现的 bug：注释写明「只上报按下」，但空分支实际仍会发出一条形似按下的报告，已修（`src/io/keys.rs`，SGR 除外：它用 `m` 正确编码松开） |
| Shift 按下时鼠标事件留给本地选区 | 已验证 | 已验证 | `tests::compat_gui::sgr_mouse_report_reaches_the_program` 内的 `forward_mouse(..., shift=true)` 返回 false |
| tmux 开 mouse 后启用 SGR 报告 | 已验证\* | 已验证\* | `tests::compat_pty::tmux_starts_and_enables_sgr_mouse`（tmux 3.4，`set -g mouse on`，PTY 模式位出现 `SGR_MOUSE`） |
| vim 的鼠标交互 | 未验证 | 未验证 | vim 本机未装 |
| htop 的鼠标/键盘交互 | 未验证 | 未验证 | htop 本机未装 |

### 颜色、光标、宽字符、字体

| 用例 | X11 | Wayland | 证据 |
| --- | --- | --- | --- |
| 256 色（`38;5;n` / `48;5;n`） | 已验证\* | 已验证\* | `tests::compat::truecolor_and_256color_reach_the_frame`：帧颜色等于调色板 `indexed(196)` / `indexed(21)` |
| true color（`38;2;r;g;b` / `48;2;...`） | 已验证\* | 已验证\* | 同上：帧颜色精确等于 `0x0A141E` / `0x010203` |
| 光标形状 DECSCUSR（`1/2 q` 方块、`3/4 q` 下划线、`5/6 q` 竖线） | 已验证\* | 已验证\* | `tests::compat::cursor_shapes_follow_decscusr`；形状与位置一并断言 |
| 宽字符：CJK 双宽、Emoji、混合行光标列对齐 | 已验证\* | 已验证\* | P1：`tests::vt` 的 CJK/Emoji/组合字符/Nerd Font 测试；P4 补 `tests::compat::mixed_width_text_keeps_cursor_columns_aligned`（`中😀a` 占 5 列，spacer 与光标列都对） |
| 字体回退（缺字时换字体） | 未验证 | 未验证 | 无显示器，GPUI 的实际字形选择看不到；P1 只验证了网格列宽，不验证字形 |
| Nerd Font 图标的实际渲染效果 | 未验证 | 未验证 | 同上；P1 只验证了网格列宽 |

### 环境与尺寸

| 用例 | X11 | Wayland | 证据 |
| --- | --- | --- | --- |
| `TERM` / `COLORTERM` 实际值 | 已验证\* | 已验证\* | `tests::compat_pty::terminal_env_is_what_programs_see`（真实 PTY 里 `$TERM`/`$COLORTERM`/`$TERM_PROGRAM`/`$TERM_PROGRAM_VERSION` = `xterm-256color` / `truecolor` / `siskin` / `0.2.0`）。值由 `src/domain/shell_spec.rs::terminal_env()` 显式设置，不继承父进程 |
| resize 后 `stty size` 与应用行为 | 已验证\* | 已验证\* | `tests::compat_pty::resize_reports_new_size_and_output_keeps_flowing`（40×10 → 60×15，`stty size` 报 `15 60`，网格 15 行，之后输出继续流动）；已有 `tests::pty::resize_updates_the_pty_window_size`（100×20 → `20 100`） |
| Bash 交互 | 已验证\* | 已验证\* | P3 `docs/shell-integration.md`：Bash 5.2.21 集成片段真实验证；日常 `sh`（dash）交互由全部 PTY 测试覆盖 |
| Zsh 交互 | 已验证\* | 已验证\* | `tests::compat_pty::zsh_runs_under_the_pty`（Zsh 5.9 在 PTY 内输出 `$ZSH_VERSION`）；P3 文档另有交互式验证 |
| Fish 交互 | 未验证 | 未验证 | Fish 本机未装 |

### 输入法与桌面

| 用例 | X11 | Wayland | 证据 |
| --- | --- | --- | --- |
| 输入法（预编辑、提交、候选框位置） | 未验证 | 未验证 | 无显示器、无输入法服务；IME 只在模型层有 `marked` 预编辑串，桌面行为未验证 |
| 全局菜单（D-Bus dbusmenu）导出 | 未验证 | 未验证 | 有会话总线时的往返由 `src/io/dbusmenu/tests.rs::protocol_round_trip_on_an_isolated_bus` 覆盖；真实桌面面板上的菜单未验证 |
| 分屏分隔线拖动、分屏下输入法候选框 | 未验证 | 未验证 | P3 已注明；无显示器跑不了 |
| 标签拖拽、双击改名的真实手感 | 未验证 | 未验证 | P3 已注明；模型层与 GPUI 模拟手势已覆盖（`tests::tabs`） |

## 异常恢复

| 场景 | 结果 | 证据 |
| --- | --- | --- |
| shell 不存在 | `ShellSpec::resolve` 返回 Err（文案含路径），`Pty::spawn` 返回 `PtyError::Shell`，不 panic、不卡死 | `tests::compat::missing_shell_is_an_error_not_a_panic` |
| PTY 派生失败（fork 层） | 未验证 | 校验失败路径已覆盖（上一行）；`fork` 本身的失败无法在不破坏测试环境的情况下人为触发，没有编造的失败注入 |
| 子进程被信号杀死（`kill -9 $$`） | 上报 `ChildExit(None)`——没有退出码就如实报空，不谎报 0 | `tests::compat_pty::signal_kill_is_reported_as_no_code_not_zero` |
| 子进程 `exit 137` | 上报 `ChildExit(Some(137))`，原样上报 | `tests::compat_pty::exit_137_is_reported_verbatim`（`exit 3` 由已有 `tests::pty::exit_status_is_reported_through_events` 覆盖） |
| D-Bus 不可用（无会话总线） | `dbusmenu::install` 的工作线程在 `Builder::session()` 失败时发 `MenuEvent::State(MenuState::Unavailable)`，应用继续跑，只是没有全局菜单；`install_menu` 里线程启动失败也只打日志，不崩溃 | `tests::compat::dbus_unavailable_is_reported_instead_of_crashing`（无 `DBUS_SESSION_BUS_ADDRESS` 时跑；有总线时跳过，有总线的往返见 `io::dbusmenu` 隔离总线测试） |
| 配置文件损坏（非法 JSON） | `FileStore::load` 返回 Err；`load_preferences` 用默认值启动，**不覆盖原文件**（`preferences_loaded=false` 时保存被挡住），状态栏留「保存失败」提示 | `tests::compat::corrupt_preferences_fall_back_without_losing_the_file`（文件层）与已有 `tests::layout::settings_save_state_and_invalid_file_preservation_are_truthful`（应用层：损坏后改设置再保存仍是失败、文件原样保留） |
| 配置文件超限（>64 KiB） | 解析前直接拒绝 | 同上，文件层测试 |
| 单条会话快照损坏 | 只丢那一条，好条目与主题/字体/shell 选择都保留（P4 新增宽容反序列化） | 同上，文件层与 `tests::compat::corrupt_memory_store_is_rejected_without_rewriting` |
| 快照段根本不是数组 | 整份失败，走默认回退（与损坏文件同路） | 同上，文件层测试 |
| 窗口关闭（模型层） | 关闭确认、取消不丢会话、关最后一个会话请求退出，均已覆盖；不 panic | 已有 `tests::layout::close_confirmation_uses_stable_session_ids_and_cancel_is_non_destructive` 与 `tests::layout::closing_the_last_session_requests_exit_without_restarting` |
| 窗口关闭（真实 OS 窗口按钮） | 未验证 | 无显示器；`main.rs` 里 `on_window_should_close` → `request_exit` → `flush_preferences` 这条路没跑过 |

## 内存上限（审计表）

| 结构 | 上限 | 淘汰/处理 | 证据 |
| --- | --- | --- | --- |
| 滚动历史 | 10,000 行 | 网格滚动区满了丢最老行（`SCROLLBACK_LINES`，`src/app/session.rs`） | `tests::compat_pty::seq_1m_history_is_capped_and_memory_growth_is_bounded`：`seq 1 1000000` 跑完 `history_size() == 10,000` |
| 整形缓存 | 512 项 / 4 MiB，LRU | 条目数或字节数任一触顶即淘汰最久未用项（P1，`src/ui/grid/cache.rs`） | P1 已有缓存命中/淘汰/键变化测试 |
| 事件队列 | **无界**（有意，P2 已评估） | 发送端 `try_send`；每轮（默认 16ms）排空；事件很小（`Wakeup` 是空变体），`seq 1M` 全程只产生 134 个事件，队列深度由一轮内的 PTY 读取次数决定，不由总输出量决定。风险：消费端若长期停转，队列会随输出增长；缓解是每轮必排空、一轮不丢事件（有界通道满了会静默丢标题/剪贴板/退出，那是更坏的结果） | P2 `tests/pty_drain.rs`；`src/app/events.rs` 头部注释 |
| 剪贴板写队列 | 64 条，丢最老 | 同步时按序写进系统剪贴板，只有最后一条真正生效，丢旧的不影响最终内容（**P4 新增上限**：此前是无界 `Vec`，恶意程序用 OSC 52 洪泛可让它随输出增长） | `tests::compat::memory_limits_are_declared_constants`；`src/app/events.rs::push_clipboard_write` |
| 历史搜索结果 | 1,000 条，截断 | `MAX_HISTORY_MATCHES`，超了停在 1000 条（P3） | 已有 `io::history_search::tests::history_search_caps_at_one_thousand_matches` |
| OSC 133 命令记录 | 100 条/会话，丢最老 | `MAX_COMMAND_SPANS`（P3） | 已有 `domain::shell_marks::tests::the_log_is_bounded_and_drops_the_oldest` |
| 会话数 | **无上限**（有意） | 用户一个个开，窗口关闭即释放 PTY 与快照；不做武断的数量上限 | 无测试——这是设计陈述，不是测量结果 |
| 配置文件 | 64 KiB | 超了直接拒绝加载（`MAX_BYTES`） | `tests::compat::corrupt_preferences_fall_back_without_losing_the_file` |
| 会话快照（恢复列表） | 由 64 KiB 配置文件间接受限 | 同上 | 同上 |
| 帧快照 | 每会话一份 `Arc<Frame>`，共享不拷贝 | 刷新时换指针，旧快照随引用释放（P1） | P1 `Arc::ptr_eq` 与快照不变性测试 |

### 实测内存（`seq 1 1000000`，`/proc/self/status` 的 VmRSS）

| 条件 | RSS 增长 | 说明 |
| --- | --- | --- |
| 单跑 | 17,720 KiB（≈17.3 MiB） | 只有内存测试在跑 |
| 全套测试并行 | 32,584 KiB（≈31.8 MiB） | 同进程里的其它 PTY 测试会分摊进来，是噪声不是泄漏 |
| 测试上界 | 64 MiB | 全套实测的 2 倍取整；历史行数断言是硬上限（必须恰好 10,000 行） |

审计结论：除剪贴板队列外，没有发现随输出无限增长的结构；剪贴板
队列已加 64 条上限。事件队列与会话数是评估后保留的无界设计，
理由与风险见上表，不硬造上限。

## 打包验证（2026-11-11，本机）

- 产物：`siskin_0.2.0_amd64.deb`（9,040,694 字节，`Installed-Size: 31040`），
  由 `scripts/build_deb.py --skip-build`（release 二进制已提前编好）打出。
- `dpkg-deb --info`：包名/版本/架构/`Depends`（`libc6`、`libxcb1`、
  `libxkbcommon*`、`fonts-noto-cjk`、`hicolor-icon-theme`、`libvulkan1` 等）核对无误。
- `dpkg-deb --contents`：41 个目录全是 `drwxr-xr-x`，19 个文件 `0644`，
  二进制 1 个 `0755`；`/usr/bin/siskin`、`.desktop` 入口、11 种尺寸图标
  （含 5 种 `@2` 高分屏）与文档都在应有的位置。
- 解包校验：`dpkg-deb -x` 到临时目录，`Exec=siskin %F`、`TryExec=siskin`、
  `Icon=io.github.fzf.Siskin` 存在，`siskin --version` 打印 `siskin 0.2.0`。
- 真实安装：`dpkg -i --force-depends` 安装成功，`dpkg -s` 为
  `install ok installed`，`/usr/bin/siskin`、`io.github.fzf.Siskin.desktop`、
  `48x48` 图标全部落位，`siskin --version` 正常。随后 `dpkg -r` 卸载，
  三处文件全部消失；再 `dpkg --purge` 清掉残留状态。
  `--force-depends` 的唯一原因是声明依赖里的 `librsvg2-common`
  本机未装（它是 SVG 加载器，我们只用 PNG 图标）；其它声明依赖均已安装。
  不用 apt 拉它——那是另一个发行版环境的事。
- `desktop-file-validate`：未验证——本机未安装，`build_deb.py` 原样跑
  会因此拒绝构建（已验证这个行为）；打包时跳过该步并记录在案。
- `lintian`：未验证——本机未安装。
- 打包脚本修了两处 P4 验证中发现的问题（都在 `scripts/build_deb.py`）：
  1. 本机 umask `0007` 且各级目录是 `2770`（setgid），新目录继承出
     `2770` 的 `DEBIAN` 目录，`dpkg-deb` 拒收（要求 0755..0775）——
     现显式 `chmod(0o755)`。
  2. 同一毛病会让装出来的数据目录带 2770——打包前把 stage 内所有目录
     （含根）统一收成 `0755`。

尚未在真实 Ubuntu/Debian 桌面上安装验证：上面是在 Ubuntu 24.04.5
的本机上做的 `dpkg -i`，没有显示器所以装完没有点开窗口验证。

## P4 修掉的两个 bug（回归测试发现的）

1. `src/io/keys.rs::mouse_report`：纯点击模式（1000，无拖拽/移动/SGR）
   的松开应不报，注释也这样写，但空分支实际仍会发出一条形似按下的
   X10 报告。已改为返回 `None`；SGR 模式不受影响（`m` 能正确编码松开）。
   测试：`tests::compat::mouse_report_x10_release_is_suppressed_in_click_only_mode`。
2. `src/io/links.rs`：注释说 `file://`「只在路径存在时才算有效」，代码
   实际不检查存在性、直接交给系统 opener。注释已改成与代码一致；
   没改点击行为。

## P4 补的两处上限/容错（不是重构）

1. `src/app/events.rs`：剪贴板写队列加 64 条上限（`MAX_CLIPBOARD_WRITES`），
   超了丢最老。
2. `src/io/preferences/mod.rs`：`sessions` 快照数组改宽容反序列化，
   单条损坏只丢那一条，不拖垮整份配置。

## 已知未完成（说清楚，不藏）

- X11/Wayland 下的窗口呈现、字体回退实际效果、输入法、分屏/标签的
  真实手感、真实桌面菜单：全部未验证，等有显示器的环境。
- Fish、vim、htop：本机未装，未验证。
- `desktop-file-validate` 与 `lintian`：本机未装，未验证。
- PTY 派生 `fork` 层本身的失败注入：未验证（见异常恢复表）。
- OSC 8 `http(s)` 链接的真实打开：未在自动化里触发 `xdg-open`。
- GUI 性能基线 S1–S7：仍未测（`docs/perf-baseline.md` 保持「未测」，
  与 P0 的约定一致）。
