<!--
@Time    : 2026/11/5 20:02
@Author  : fzf
@FileName: CHANGELOG.md
@Software: Siskin
-->

# 变更记录

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
