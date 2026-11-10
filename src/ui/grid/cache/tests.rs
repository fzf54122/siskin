// @Time    : 2026/11/9 16:50
// @Author  : fzf
// @FileName: tests.rs
// @Software: Siskin

use super::*;
use crate::io::vt::CellStyle;
fn style(bold: bool) -> CellStyle {
    CellStyle {
        fg: 0xffffff,
        bold,
        ..CellStyle::default()
    }
}

#[test]
fn the_key_covers_every_input_that_changes_shaping() {
    let family: SharedString = "monospace".into();
    let base = ShapeKey::new(&family, "abc", style(false), 13., 1.);
    let different_family = ShapeKey::new(&"other".into(), "abc", style(false), 13., 1.);
    let different_text = ShapeKey::new(&family, "abd", style(false), 13., 1.);
    let different_weight = ShapeKey::new(&family, "abc", style(true), 13., 1.);
    let different_italic = ShapeKey::new(
        &family,
        "abc",
        CellStyle {
            italic: true,
            ..style(false)
        },
        13.,
        1.,
    );
    let different_underline = ShapeKey::new(
        &family,
        "abc",
        CellStyle {
            underline: true,
            ..style(false)
        },
        13.,
        1.,
    );
    let different_size = ShapeKey::new(&family, "abc", style(false), 14., 1.);
    let different_scale = ShapeKey::new(&family, "abc", style(false), 13., 2.);
    let different_color = ShapeKey::new(
        &family,
        "abc",
        CellStyle {
            fg: 0x000000,
            ..style(false)
        },
        13.,
        1.,
    );
    for key in [
        different_family,
        different_text,
        different_weight,
        different_italic,
        different_underline,
        different_size,
        different_scale,
        different_color,
    ] {
        assert_ne!(base, key, "缓存键漏掉了影响整形的输入：{key:?}");
    }
}

#[test]
fn eviction_keeps_entry_and_byte_counts_bounded() {
    let family: SharedString = "cache-eviction-probe".into();
    let mut cache = ShapeCache::new();
    // 灌满一倍余量：条目数与字节数都不能越过上限。
    for index in 0..(MAX_ENTRIES + 200) {
        let text = format!("probe-{index}");
        let key = ShapeKey::new(&family, &text, style(false), 13., 1.);
        cache.insert_at(key, Arc::new(gpui::ShapedLine::default()), index as u64 + 1);
    }
    assert!(cache.entries.len() <= MAX_ENTRIES, "条目数超过上限");
    let recomputed: usize = cache.entries.values().map(|entry| entry.cost).sum();
    assert_eq!(cache.bytes, recomputed, "字节统计与键成本不一致");
    assert!(cache.bytes <= MAX_BYTES);
    let max_text = "probe-".len() + "512".len();
    assert!(
        cache.bytes
            <= cache.entries.len()
                * (max_text.saturating_mul(4)
                    + max_text.saturating_mul(32)
                    + family.len()
                    + std::mem::size_of::<CacheEntry>()
                    + std::mem::size_of::<ShapeKey>())
                .max(64),
        "字节统计失真"
    );
    // 淘汰完之后仍然可用：再插一条能进得去。
    cache.insert_at(
        ShapeKey::new(&family, "probe-final", style(false), 13., 1.),
        Arc::new(gpui::ShapedLine::default()),
        (MAX_ENTRIES + 202) as u64,
    );
    assert!(cache.entries.len() <= MAX_ENTRIES);
}

#[test]
fn recently_used_entries_survive_lru_eviction() {
    let family: SharedString = "cache-lru".into();
    let mut cache = ShapeCache::new();
    let old = ShapeKey::new(&family, "old", style(false), 13., 1.);
    let recent = ShapeKey::new(&family, "recent", style(false), 13., 1.);
    cache.insert_at(old.clone(), Arc::new(gpui::ShapedLine::default()), 1);
    cache.insert_at(recent.clone(), Arc::new(gpui::ShapedLine::default()), 2);
    // 触碰 old，使 recent 成为 LRU。
    cache.clock = 3;
    if let Some(entry) = cache.entries.get_mut(&old) {
        ShapeCache::touch(entry, cache.clock);
    }
    assert!(cache.evict_one());
    assert!(cache.entries.contains_key(&old));
    assert!(!cache.entries.contains_key(&recent));
}

#[test]
fn oversized_entry_is_skipped_without_evicting_useful_entries() {
    let family: SharedString = "cache-oversized".into();
    let mut cache = ShapeCache::new();
    let existing = ShapeKey::new(&family, "kept", style(false), 13., 1.);
    cache.insert_at(existing.clone(), Arc::new(gpui::ShapedLine::default()), 1);
    let bytes_before = cache.bytes;
    let oversized = ShapeKey::new(
        &family,
        &"x".repeat(MAX_BYTES / 2 + 1),
        style(false),
        13.,
        1.,
    );
    cache.insert_at(oversized, Arc::new(gpui::ShapedLine::default()), 2);
    assert!(cache.entries.contains_key(&existing));
    assert_eq!(cache.bytes, bytes_before);
    assert!(cache.bytes <= MAX_BYTES);
}

#[test]
fn byte_limit_evicts_entries_before_the_entry_limit() {
    let family: SharedString = "cache-byte-budget".into();
    let mut cache = ShapeCache::new();
    for index in 0..20 {
        let text = format!("{index}-{}", "x".repeat(20_000));
        let key = ShapeKey::new(&family, &text, style(false), 13., 1.);
        cache.insert_at(key, Arc::new(gpui::ShapedLine::default()), index as u64 + 1);
    }
    assert!(cache.entries.len() < MAX_ENTRIES);
    assert!(cache.bytes <= MAX_BYTES);
    assert_eq!(
        cache.bytes,
        cache
            .entries
            .values()
            .map(|entry| entry.cost)
            .sum::<usize>(),
        "字节淘汰后统计值必须精确"
    );
}

#[test]
fn an_existing_key_is_not_charged_twice() {
    let family: SharedString = "cache-dedupe".into();
    let mut cache = ShapeCache::new();
    let key = ShapeKey::new(&family, "same", style(false), 13., 1.);
    for _ in 0..4 {
        cache.insert_at(key.clone(), Arc::new(gpui::ShapedLine::default()), 1);
    }
    assert_eq!(cache.entries.len(), 1, "重复插入把同一条键算进了占用");
}

/// 命中率必须真实为正，否则缓存等于没开。这条走真实整形路径，
/// 不是手改计数：同一段文本申请十次，第一次未命中、之后九次命中。
#[gpui::test]
fn repeated_shaping_of_the_same_text_hits_the_cache(cx: &mut gpui::TestAppContext) {
    let (_, cx) = cx.add_window_view(|_, _| TestAnchor);
    let family: SharedString = "monospace".into();
    let mut cache = ShapeCache::new();
    // 第一轮：全部未命中，各自调一次 shape_line。
    cx.update(|window, _| {
        for index in 0..10 {
            let text = format!("line-{index}");
            let line = cache.line(&family, &text, style(false), 13., window);
            assert!(!line.text.is_empty(), "整形结果应有文本");
        }
        assert_eq!(cache.stats(), (0, 10), "首次整形必须全部未命中");
        // 第二轮同样内容：全部命中，不再调 shape_line。
        for index in 0..10 {
            let text = format!("line-{index}");
            let _ = cache.line(&family, &text, style(false), 13., window);
        }
        assert_eq!(cache.stats(), (10, 10), "同样内容再次整形必须全部命中");
    });
    assert_eq!(cache.entries.len(), 10);
}
