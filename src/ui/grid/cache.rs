// @Time    : 2026/11/9 15:20
// @Author  : fzf
// @FileName: cache.rs
// @Software: Siskin

//! 整形结果缓存。`shape_line` 是整行渲染里最贵的一步，而终端每一帧都在
//! 重新整形同一份文本：同一行 shell 输出、同一行提示符，滚动时更是反复
//! 出现。缓存把这些重复整形降成一次哈希查找。
//!
//! 有界：条目数与估算字节数双上限，滚动 1 万行历史也不会无限增长。
//! 失效靠完整缓存键，不靠显式清理：字体、字号、样式任何一个变化，
//! 键就不同，旧条目自然被淘汰，不会把上一套字形的结果画到新字体上。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use gpui::SharedString;

use crate::io::vt::CellStyle;

use super::shape;

/// 条目数上限。一行可视区约 60 行 × 每行 4 段 run，512 足够覆盖
/// 两帧的完整内容再加一倍余量，不会把内存堆到几十 MB。
const MAX_ENTRIES: usize = 512;
/// 估算字节数上限。整形结果的字节数无法精确得知，按文本长度粗估，
/// 512 条目 × 平均 60 字节远小于这个上限，实际由条目数先触顶。
const MAX_BYTES: usize = 4 << 20;

/// 缓存键。每一个参与整形结果的输入都必须在键里：
/// 内容、字体族、字重、字形样式、字号、前景色、下划线。
/// 少了任何一项，换字体或改色之后都会拿到旧结果。
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ShapeKey {
    /// 文本内容。同一行里不同 run 的文本不会相同。
    pub text: Box<str>,
    /// 字体族名（解析后的真实字体名也由它唯一决定）。
    pub family: SharedString,
    /// 是否加粗。
    pub bold: bool,
    /// 是否斜体。
    pub italic: bool,
    /// 是否下划线（下划线颜色跟前景色走，所以不再单列颜色）。
    pub underline: bool,
    /// 字号。DPi / scale 变化必然伴随字号变化，所以不额外存 scale。
    pub font_size_bits: u32,
    /// GPUI 窗口 scale。字号以逻辑像素表示，scale 独立变化时也必须失效。
    pub scale_bits: u32,
    /// 前景色。
    pub fg: u32,
}

impl ShapeKey {
    pub(crate) fn new(
        family: &SharedString,
        text: &str,
        style: CellStyle,
        font_size: f32,
        scale: f32,
    ) -> Self {
        Self {
            text: text.into(),
            family: family.clone(),
            bold: style.bold,
            italic: style.italic,
            underline: style.underline,
            font_size_bits: font_size.to_bits(),
            scale_bits: scale.to_bits(),
            fg: style.fg,
        }
    }

    /// 一条缓存的估算成本，包括文本、字体族、索引/节点和结果的粗略预算。
    fn cost(&self) -> usize {
        self.text
            .len()
            .saturating_mul(4)
            .saturating_add(self.text.chars().count().saturating_mul(32))
            .saturating_add(self.family.len())
            .saturating_add(std::mem::size_of::<CacheEntry>())
            .saturating_add(std::mem::size_of::<ShapeKey>())
            .max(64)
    }
}

/// LRU 条目。`last_used` 只在命中时更新，淘汰时扫描最多 512 项。
struct CacheEntry {
    line: Arc<gpui::ShapedLine>,
    last_used: u64,
    cost: usize,
}

pub(crate) struct ShapeCache {
    entries: HashMap<ShapeKey, CacheEntry>,
    bytes: usize,
    hits: u64,
    misses: u64,
    clock: u64,
}

impl ShapeCache {
    pub(crate) fn new() -> Self {
        Self {
            entries: HashMap::new(),
            bytes: 0,
            hits: 0,
            misses: 0,
            clock: 0,
        }
    }

    /// 取缓存，未命中就整形后写入。调用方不需要知道是否命中：
    /// 命中与否只影响耗时，不影响输出。
    pub(crate) fn line(
        &mut self,
        family: &SharedString,
        text: &str,
        style: CellStyle,
        font_size: f32,
        window: &mut gpui::Window,
    ) -> Arc<gpui::ShapedLine> {
        let key = ShapeKey::new(family, text, style, font_size, window.scale_factor());
        self.clock = self.clock.wrapping_add(1);
        if let Some(entry) = self.entries.get_mut(&key) {
            Self::touch(entry, self.clock);
            self.hits += 1;
            return Arc::clone(&entry.line);
        }
        self.misses += 1;
        let line = shape::shape_text(family, text, style, font_size, window);
        self.insert_at(key, Arc::clone(&line), self.clock);
        line
    }

    fn touch(entry: &mut CacheEntry, clock: u64) {
        entry.last_used = clock;
    }

    fn insert_at(&mut self, key: ShapeKey, line: Arc<gpui::ShapedLine>, last_used: u64) {
        if let Some(entry) = self.entries.get_mut(&key) {
            Self::touch(entry, last_used);
            return;
        }
        let cost = key.cost();
        // 单个过大的 run 不进入缓存，避免清空缓存后仍突破字节上限。
        if cost > MAX_BYTES {
            return;
        }
        // 先给新条目腾位：条目数或字节数超了就淘汰最近最少使用项。
        while (self.entries.len() >= MAX_ENTRIES || self.bytes.saturating_add(cost) > MAX_BYTES)
            && self.evict_one()
        {}
        if self.entries.len() >= MAX_ENTRIES || self.bytes.saturating_add(cost) > MAX_BYTES {
            return;
        }
        self.bytes += cost;
        self.entries.insert(
            key,
            CacheEntry {
                line,
                last_used,
                cost,
            },
        );
    }

    /// 淘汰最近最少使用的一条。返回 `false` 表示表已空、无处可淘汰。
    fn evict_one(&mut self) -> bool {
        let Some(least_used) = self
            .entries
            .iter()
            .min_by_key(|(_, entry)| entry.last_used)
            .map(|(key, _)| key.clone())
        else {
            return false;
        };
        let removed = self.entries.remove(&least_used);
        if let Some(entry) = &removed {
            self.bytes = self.bytes.saturating_sub(entry.cost);
        }
        removed.is_some()
    }

    #[cfg(test)]
    fn stats(&self) -> (u64, u64) {
        (self.hits, self.misses)
    }
}

/// 进程级共享缓存。整形只依赖文本系统，与哪个窗口无关；多窗口共享
/// 同一份缓存不会串结果，因为键里带了字体与字号。
pub(crate) fn shared() -> &'static Mutex<ShapeCache> {
    static CACHE: std::sync::OnceLock<Mutex<ShapeCache>> = std::sync::OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(ShapeCache::new()))
}

/// 走缓存整形。锁只在一次整形内持有，整形本身不跨 await、不递归加锁。
pub(crate) fn shape_cached(
    family: &SharedString,
    text: &str,
    style: CellStyle,
    font_size: f32,
    window: &mut gpui::Window,
) -> Arc<gpui::ShapedLine> {
    match shared().lock() {
        Ok(mut cache) => cache.line(family, text, style, font_size, window),
        // 锁中毒说明有线程在持锁时 panic。缓存是纯性能优化，
        // 退化到直接整形比把界面一起拖死要好。
        Err(_) => shape::shape_text(family, text, style, font_size, window),
    }
}

/// 空视图。这里只需要一个窗口来驱动真实文本系统，不渲染任何东西。
#[cfg(test)]
struct TestAnchor;

#[cfg(test)]
impl gpui::Render for TestAnchor {
    fn render(
        &mut self,
        _: &mut gpui::Window,
        _: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        gpui::div()
    }
}

#[cfg(test)]
mod tests;
