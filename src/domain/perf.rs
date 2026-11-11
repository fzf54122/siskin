// @Time    : 2026/11/10 18:00
// @Author  : fzf
// @FileName: perf.rs
// @Software: Siskin

//! P0 性能基线：可开关的采样与汇总。纯数据层，不依赖 GPUI。
//!
//! 只记录数字，不记录终端内容、命令行、剪贴板或任何用户数据。
//! 默认关闭，开启后每帧只做一次 `VecDeque` 入队与几次整数累加，
//! 汇总（排序求 P50/P95）只在读 overlay 或日志时才算。
//!
//! 诚实说明：这里的「帧」是一次 PTY 事件轮次 + `cx.notify()` 触发的
//! 渲染，`frame_time` 是轮次耗时，不是 GPU 呈现耗时。固定 revision
//! 的 GPUI 没有可直接调用的帧 profiler（见 `docs/perf-baseline.md`），
//! 所以不为追 profiler 升级框架，先用可复现的轮次指标做基线。

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::Duration;

/// 采样窗口上限。60fps 下约 10 秒，排序求分位数的成本可忽略。
const MAX_SAMPLES: usize = 600;

/// 一次更新/渲染轮次的数字快照。
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct PerfSample {
    /// 本轮 `drain_events` 耗时（含构帧），不是 GPU 呈现耗时。
    pub frame_time: Duration,
    /// 本轮 prepaint 整形耗时（`shape_row` 全行合计）。
    pub shape_time: Duration,
    /// 本轮所有脏会话 `Session::refresh` 耗时合计。
    pub frame_build_time: Duration,
    /// 本轮从事件通道排空的 PTY 事件数。
    pub pty_events: u32,
    /// 本轮真正重构了 Frame 的会话数。
    pub sessions_refreshed: u32,
    /// 本轮 prepaint 产出的绘制任务数（run + 背景 + 底线）。
    pub draw_tasks: u32,
    /// 记录本轮时整形缓存的累计命中数。
    pub shape_hits: u64,
    /// 记录本轮时整形缓存的累计未命中数。
    pub shape_misses: u64,
}

impl PerfSample {
    pub(crate) fn shape_hit_rate(&self) -> f64 {
        let total = self.shape_hits + self.shape_misses;
        if total == 0 {
            0.
        } else {
            self.shape_hits as f64 / total as f64
        }
    }
}

/// 汇总。所有时间都以毫秒计，`fps` 由窗口内样本数 / 窗口时长推导。
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct PerfSummary {
    pub frames: usize,
    pub fps: f64,
    pub frame_p50_ms: f64,
    pub frame_p95_ms: f64,
    pub frame_max_ms: f64,
    pub shape_avg_ms: f64,
    pub frame_build_avg_ms: f64,
    pub pty_events_per_frame: f64,
    pub sessions_per_frame: f64,
    pub draw_tasks_per_frame: f64,
    pub shape_hit_rate: f64,
}

impl PerfSummary {
    /// 一行 overlay 文本。只含数字，不含任何终端内容。
    /// 开头带样本数，方便判断 P95 是算了 10 帧还是 600 帧。
    pub(crate) fn one_line(&self) -> String {
        format!(
            "n={} FPS {:.1} · p50 {:.1}ms · p95 {:.1}ms · max {:.1}ms · shape {:.2}ms · build {:.2}ms · evt {:.1} · sess {:.1} · draw {:.0} · cache {:.0}%",
            self.frames,
            self.fps,
            self.frame_p50_ms,
            self.frame_p95_ms,
            self.frame_max_ms,
            self.shape_avg_ms,
            self.frame_build_avg_ms,
            self.pty_events_per_frame,
            self.sessions_per_frame,
            self.draw_tasks_per_frame,
            self.shape_hit_rate * 100.
        )
    }
}

/// 采样器。关闭时 `record` 直接返回，不入队、不分配。
pub(crate) struct PerfRecorder {
    enabled: bool,
    samples: VecDeque<PerfSample>,
}

impl PerfRecorder {
    pub(crate) fn new(enabled: bool) -> Self {
        Self {
            enabled,
            samples: VecDeque::new(),
        }
    }

    pub(crate) fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub(crate) fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.samples.clear();
        }
    }

    pub(crate) fn record(&mut self, sample: PerfSample) {
        if !self.enabled {
            return;
        }
        if self.samples.len() >= MAX_SAMPLES {
            self.samples.pop_front();
        }
        self.samples.push_back(sample);
    }

    /// 把 prepaint 的整形耗时、绘制任务数与缓存命中数补进最近一条样本。
    /// prepaint 发生在 `notify()` 之后，注定比轮次晚到，所以用
    /// 「补最近一条」而不是另起一条，否则 FPS 会被虚增一倍。
    pub(crate) fn note_render(
        &mut self,
        shape_time: Duration,
        draw_tasks: u32,
        shape_hits: u64,
        shape_misses: u64,
    ) {
        if !self.enabled {
            return;
        }
        if let Some(last) = self.samples.back_mut() {
            last.shape_time = shape_time;
            last.draw_tasks = draw_tasks;
            last.shape_hits = shape_hits;
            last.shape_misses = shape_misses;
        }
    }

    pub(crate) fn summary(&self) -> PerfSummary {
        if self.samples.is_empty() {
            return PerfSummary::default();
        }
        let frames = self.samples.len();
        let mut times: Vec<f64> = self.samples.iter().map(ms).collect();
        times.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let total = times.iter().sum::<f64>();
        PerfSummary {
            frames,
            fps: frames as f64 / (total / 1000.).max(1e-9),
            frame_p50_ms: percentile(&times, 0.50),
            frame_p95_ms: percentile(&times, 0.95),
            frame_max_ms: *times.last().unwrap_or(&0.),
            shape_avg_ms: avg(&self.samples, |s| s.shape_time),
            frame_build_avg_ms: avg(&self.samples, |s| s.frame_build_time),
            pty_events_per_frame: avg_u32(&self.samples, |s| s.pty_events),
            sessions_per_frame: avg_u32(&self.samples, |s| s.sessions_refreshed),
            draw_tasks_per_frame: avg_u32(&self.samples, |s| s.draw_tasks),
            shape_hit_rate: self.samples.back().map_or(0., PerfSample::shape_hit_rate),
        }
    }
}

fn ms(sample: &PerfSample) -> f64 {
    sample.frame_time.as_secs_f64() * 1000.
}

fn avg(samples: &VecDeque<PerfSample>, pick: fn(&PerfSample) -> Duration) -> f64 {
    if samples.is_empty() {
        return 0.;
    }
    let total: f64 = samples.iter().map(|s| pick(s).as_secs_f64() * 1000.).sum();
    total / samples.len() as f64
}

fn avg_u32(samples: &VecDeque<PerfSample>, pick: fn(&PerfSample) -> u32) -> f64 {
    if samples.is_empty() {
        return 0.;
    }
    let total: u64 = samples.iter().map(|s| u64::from(pick(s))).sum();
    total as f64 / samples.len() as f64
}

fn percentile(sorted_ms: &[f64], ratio: f64) -> f64 {
    if sorted_ms.is_empty() {
        return 0.;
    }
    let index = ((sorted_ms.len() as f64 - 1.) * ratio).round() as usize;
    sorted_ms[index.min(sorted_ms.len() - 1)]
}

/// 进程级共享采样器。`app`、`ui/grid` 与命令面板都经由它读写，
/// 不必把采样器塞进 `GridInputs` 再传一遍。
pub(crate) fn shared() -> &'static Mutex<PerfRecorder> {
    static RECORDER: std::sync::OnceLock<Mutex<PerfRecorder>> = std::sync::OnceLock::new();
    RECORDER.get_or_init(|| Mutex::new(PerfRecorder::new(enabled_from_env())))
}

/// `SISKIN_PERF=1` / `true` / `on` 开启，其余值与未设置都关闭。
/// 解析只看字符串本身，不读终端内容。
pub(crate) fn enabled_from_env() -> bool {
    parse_env_flag(&std::env::var("SISKIN_PERF").ok())
}

pub(crate) fn parse_env_flag(value: &Option<String>) -> bool {
    matches!(
        value.as_deref().map(str::to_ascii_lowercase).as_deref(),
        Some("1") | Some("true") | Some("on") | Some("yes")
    )
}

pub(crate) fn set_enabled(enabled: bool) {
    if let Ok(mut recorder) = shared().lock() {
        recorder.set_enabled(enabled);
    }
}

pub(crate) fn is_enabled() -> bool {
    shared().lock().map(|r| r.is_enabled()).unwrap_or(false)
}

pub(crate) fn record(sample: PerfSample) {
    if let Ok(mut recorder) = shared().lock() {
        recorder.record(sample);
    }
}

pub(crate) fn note_render(
    shape_time: Duration,
    draw_tasks: u32,
    shape_hits: u64,
    shape_misses: u64,
) {
    if let Ok(mut recorder) = shared().lock() {
        recorder.note_render(shape_time, draw_tasks, shape_hits, shape_misses);
    }
}

pub(crate) fn summary() -> PerfSummary {
    shared().lock().map(|r| r.summary()).unwrap_or_default()
}

#[cfg(test)]
mod tests;
