// @Time    : 2026/11/4 23:30
// @Author  : fzf
// @FileName: tests.rs
// @Software: Siskin

use super::*;
use crate::theme::colors::{contrast, ensure_contrast};

#[test]
fn every_preset_has_valid_surfaces_and_paper_stays_white() {
    for theme in ThemeId::PRESETS {
        let palette = Palette::of(theme);
        if theme == ThemeId::Paper {
            assert_eq!(
                [palette.window, palette.sidebar, palette.terminal],
                [0xffffff; 3]
            );
        } else {
            assert_ne!(palette.window, palette.terminal, "{}", theme.name());
        }
        assert_eq!(palette.id, theme);
        assert!(palette.swatch().iter().all(|color| *color <= 0xffffff));
    }
}

#[test]
fn installed_fonts_are_preferred_but_never_empty() {
    let installed = vec!["DejaVu Sans Mono".to_string()];
    assert_eq!(mono_fonts(&installed), vec!["DejaVu Sans Mono"]);
    assert_eq!(mono_fonts(&[]), vec!["monospace"]);
}

#[test]
fn system_mode_follows_the_desktop_preference() {
    let custom = CustomTheme::default();
    assert_eq!(resolve(ThemeMode::System, custom, true), ThemeId::Graphite);
    assert_eq!(resolve(ThemeMode::System, custom, false), ThemeId::Paper);
    assert_eq!(resolve(ThemeMode::Light, custom, true), ThemeId::Paper);
    assert_eq!(resolve(ThemeMode::Dark, custom, false), ThemeId::Graphite);
    assert_eq!(resolve(ThemeMode::Custom, custom, false), ThemeId::Custom);
}

#[test]
fn theme_mode_cycles_through_all_choices() {
    let mut mode = ThemeMode::System;
    for _ in 0..ThemeMode::ALL.len() {
        mode = mode.next();
    }
    assert_eq!(mode, ThemeMode::System);
}

#[test]
fn custom_accent_keeps_contrast_and_marks_itself_custom() {
    for dark in [true, false] {
        for (accent, (color, _name)) in ACCENTS.iter().enumerate() {
            let palette = Palette::custom(CustomTheme {
                dark,
                accent,
                ..Default::default()
            });
            assert_eq!(palette.id, ThemeId::Custom);
            assert_eq!(palette.dark, dark);
            assert_eq!(palette.accent, *color);
            assert_ne!(palette.accent, palette.window);
            assert_ne!(palette.tint, palette.canvas);
        }
    }
}

#[test]
fn mixing_stays_inside_the_color_range() {
    // Rounds half up per channel: 127.5 becomes 128.
    assert_eq!(mix(0x000000, 0xffffff, 0.5), 0x808080);
    assert_eq!(mix(0xffffff, 0x000000, 0.), 0xffffff);
    assert_eq!(mix(0x123456, 0xffffff, 2.), 0xffffff);
}

#[test]
fn contrast_matches_the_wcag_ratio_for_black_and_white() {
    // 教科书值：纯白对纯黑是 21:1。
    let ratio = contrast(0xffffff, 0x000000);
    assert!((ratio - 21.).abs() < 0.01, "{ratio}");
    assert!((contrast(0x777777, 0x777777) - 1.).abs() < 0.001);
}

#[test]
fn ensure_contrast_lifts_text_on_dark_backgrounds() {
    let adjusted = ensure_contrast(0x222222, 0x101418, 4.5);
    assert!(
        contrast(adjusted, 0x101418) >= 4.5,
        "#{adjusted:06x} 仍然太暗"
    );
    // 已经达标时不该动，避免无谓地漂移用户选的颜色。
    assert_eq!(ensure_contrast(0xffffff, 0x101418, 4.5), 0xffffff);
}

#[test]
fn ensure_contrast_darkens_text_on_light_backgrounds() {
    let adjusted = ensure_contrast(0xf0f0f0, 0xffffff, 4.5);
    assert!(
        contrast(adjusted, 0xffffff) >= 4.5,
        "#{adjusted:06x} 仍然太淡"
    );
    assert_eq!(ensure_contrast(0x000000, 0xffffff, 4.5), 0x000000);
}

#[test]
fn white_on_white_never_survives_a_custom_theme() {
    // 用户自己填的白底 + 白字是最容易把终端设废的组合。
    let palette = Palette::custom(CustomTheme {
        dark: false,
        background: Some(ThemeColor(0xffffff)),
        foreground: Some(ThemeColor(0xffffff)),
        ..Default::default()
    });
    assert!(
        contrast(palette.ink, palette.terminal) >= 4.5,
        "白字被原样采信了：#{:06x} on #{:06x}",
        palette.ink,
        palette.terminal
    );
}

#[test]
fn dark_on_dark_never_survives_a_custom_theme() {
    let palette = Palette::custom(CustomTheme {
        dark: true,
        background: Some(ThemeColor(0x000000)),
        foreground: Some(ThemeColor(0x080808)),
        ..Default::default()
    });
    assert!(
        contrast(palette.ink, palette.terminal) >= 4.5,
        "暗字被原样采信了：#{:06x} on #{:06x}",
        palette.ink,
        palette.terminal
    );
    // 次要文字同样要能读，不能只有主文字达标。
    assert!(
        contrast(palette.faint_text, palette.terminal) >= 4.5,
        "次要文字 #{:06x} on #{:06x}",
        palette.faint_text,
        palette.terminal
    );
}

#[test]
fn a_readable_custom_foreground_is_kept_verbatim() {
    // 用户认真选过的可读颜色不该被校移动过手。
    let chosen = 0x3b3d42;
    let palette = Palette::custom(CustomTheme {
        dark: false,
        foreground: Some(ThemeColor(chosen)),
        ..Default::default()
    });
    assert_eq!(palette.ink, chosen);
}

#[test]
fn default_themes_keep_small_text_readable() {
    for palette in [PAPER, GRAPHITE] {
        for (foreground, background) in [
            (palette.ink, palette.window),
            (palette.muted, palette.window),
            (palette.faint, palette.window),
            (palette.accent, palette.tint),
            (palette.output, palette.terminal),
        ] {
            let ratio = contrast(foreground, background);
            assert!(
                ratio >= 4.5,
                "{} #{foreground:06x}/#{background:06x}: {ratio}",
                palette.id.name()
            );
        }
    }
}
