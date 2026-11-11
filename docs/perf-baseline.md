<!--
@Time    : 2026/11/10 18:10
@Author  : fzf
@FileName: perf-baseline.md
@Software: Siskin
-->

# P0 性能基线

`plan.md` 第 4 节的落地记录。先测量，再优化；没有跑过的
项目明确写「未测」，不编造数字。

## 开启方式

三选一，效果相同，默认关闭：

```sh
SISKIN_PERF=1 siskin
siskin --perf
# 已打开的窗口：Ctrl+K →「性能采样开/关」
```

开启后右上角出现一行 overlay，例如：

```text
n=600 FPS 59.2 · p50 16.8ms · p95 18.4ms · max 42.1ms · shape 1.20ms · build 0.80ms · evt 12.0 · sess 1.0 · draw 142 · cache 87%
```

关闭采样会清空已采样本，避免旧数据混进新一轮基线。

## 指标定义

| 指标 | 含义 | 来源 |
| --- | --- | --- |
| FPS | 采样窗口内「有变化的轮次」数 / 窗口总耗时 | `drain_events` 每 16ms 轮次，`changed` 时记一条 |
| p50 / p95 / max | 轮次耗时的分位数与最大值 | 同上，样本上限 600 帧（约 10 秒） |
| shape | prepaint 整形全行合计耗时 | `ui/grid/element.rs` 的 `prepaint` |
| build | 脏会话 `Session::refresh` 耗时合计 | `app/events.rs` 的 `refresh_dirty_sessions` |
| evt | 每轮排空的 PTY 事件数 | `drain_events` |
| sess | 每轮真正重构 Frame 的会话数 | 同上 |
| draw | 每轮 prepaint 的绘制任务数（run + 背景 + 底线） | `prepaint` |
| cache | 整形缓存累计命中率 | `ui/grid/cache.rs` 的 `stats_snapshot` |

实现在 `src/domain/perf.rs`，纯数据层，可单测；采样开关是
进程级共享 `Mutex<PerfRecorder>`，见 `shared()`。

## 诚实说明（先读这段再看数字）

- **轮次时间不是 GPU 呈现时间。** `frame_time` 是
  `drain_events` 的耗时（含构帧），渲染发生在随后的
  `cx.notify()`。它能暴露「构帧 / 事件合并是不是瓶颈」，
  不能证明「屏幕呈现就是这个延迟」。输出到屏幕的真实
  延迟需要另外的 present 计时，不在本轮范围。
- **空闲轮次不计入 FPS。** 空闲时 `changed` 为假，不记样本，
  否则 FPS 会被 16ms 定时器虚报到 62.5。
- **prepaint 的数字是「补进最近一条样本」。** prepaint
  发生在 `notify()` 之后，所以整形耗时与绘制任务数是
  回填的，不是另起一条样本，否则 FPS 会被虚增一倍。
- **采样本身的开销。** 关闭时 `record` 直接返回，只剩
  `drain_events` 开头一个 `Instant::now()` 与一次
  `is_enabled()` 锁检查；开启后每帧一次 `VecDeque`
  入队。汇总的排序只在读 overlay 时才算。
- **不记录用户数据。** 采样只含数字与计数，不写终端
  内容、命令行、剪贴板或路径。

## GPUI profiler

固定 revision `c427327`（`Cargo.toml`）的 GPUI 没有在
本仓库的调用面上提供可直接用的帧 / 输入延迟 profiler。
结论：不为追 profiler 升级框架，先用上述轮次指标做
基线；若后续确实需要 present 级计时，再单独评估
GPUI 的 `Window` / 渲染回调里能否取到可信时间戳。

## 固定场景

每次测基线都按同一套步骤跑，换场景只换命令，不换
窗口大小、字体与字号。记录时同时记环境：发行版、
桌面（X11 / Wayland）、GPU / 驱动、Rust 版本、窗口
尺寸、字体与字号、会话数。

| 编号 | 场景 | 步骤 |
| --- | --- | --- |
| S1 | 空闲 | 打开一个会话，不输入，采样 30 秒 |
| S2 | 持续键入 | 在 shell 里连续输入 / 删除，采样 30 秒 |
| S3 | 10,000 行 scrollback 下滚动与搜索 | `seq 1 100000` 后滚轮上下滚动，`Ctrl+Shift+F` 搜一个常见词 |
| S4 | 持续输出 | `seq 1 1000000` |
| S5 | ANSI 彩色输出 | `ls --color -R /usr` 或带颜色的 `git log --graph --oneline --all` |
| S6 | 多会话同时输出 | 开 3 个会话，同时跑 `seq 1 200000` |
| S7 | 中文 / Emoji / 组合字符 / Nerd Font | 输出一段中文、Emoji、组合字符与 Nerd Font 图标，滚动回看 |

每个场景记录 overlay 的一行数字，并写清「采样
窗口约 10 秒，p95 来自窗口内样本」。

## 基线记录

> 以下为模板：每一行都要填真实跑出来的数字与环境，
> 没跑过的场景留空并写「未测」，不要填估计值。

| 场景 | 环境（发行版 / 桌面 / 窗口 / 字体） | FPS | p50 | p95 | max | shape | build | evt/帧 | sess/帧 | draw/帧 | cache | 日期 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| S1 空闲 | 未测 | | | | | | | | | | | |
| S2 键入 | 未测 | | | | | | | | | | | |
| S3 滚动+搜索 | 未测 | | | | | | | | | | | |
| S4 `seq 1 1000000` | 未测 | | | | | | | | | | | |
| S5 彩色输出 | 未测 | | | | | | | | | | | |
| S6 多会话 | 未测 | | | | | | | | | | | |
| S7 Unicode | 未测 | | | | | | | | | | | |

本机（Muse 云端 VM）说明：该环境无显示器、无 Vulkan，
也无法跑出真实窗口，所以上述 GUI 基线**尚未在本机
测出**，不是省略，是明确未测。单元测试（`domain::perf`
的分位数、FPS 推导、容量上限、开关行为）见
`src/domain/perf/tests.rs`，随 `cargo test` 跑。

## 验收对照（plan.md §4）

- [x] 指标可在诊断模式下启用（`--perf` / `SISKIN_PERF=1` / 命令面板），并能区分整形、构帧与整轮时间。
- [x] 采样默认关闭，关闭时不给正式渲染路径加可观测开销；不写用户数据进日志。
- [x] 有一份可重复运行的性能场景与基线记录模板（上表）。
- [ ] GUI 基线数字：在真实桌面会话里按 S1–S7 跑一遍并填表。
- [ ] 每项优化后用同一套场景做前后对比，只报 P95 与最差场景，不只报平均值。
