// @Time    : 2026/11/4 23:30
// @Author  : fzf
// @FileName: tests.rs
// @Software: Siskin

use super::*;

#[test]
fn every_preset_has_distinct_surfaces() {
    for theme in ThemeId::PRESETS {
        let palette = Palette::of(theme);
        assert_ne!(palette.window, palette.terminal, "{}", theme.name());
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

fn luminance(color: u32) -> f64 {
    let channel = |shift: u32| {
        let value = ((color >> shift) & 255u32) as f64 / 255.;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    channel(16) * 0.2126 + channel(8) * 0.7152 + channel(0) * 0.0722
}

fn contrast(a: u32, b: u32) -> f64 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
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
