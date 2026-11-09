// @Time    : 2026/11/5 14:42
// @Author  : fzf
// @FileName: preferences.rs
// @Software: Siskin

//! 配置的往返与拒绝规则。全部走 `MemoryStore`，不碰真实配置目录，
//! 也不会因为本机已有配置文件而变色。

use crate::domain::ShellKind;
use crate::io::preferences::{MAX_BYTES, MemoryStore, Preferences, Store as _};
use crate::settings::{CustomTheme, Settings, ThemeMode};
use crate::theme::ThemeColor;

fn preferences() -> Preferences {
    Preferences {
        theme_mode: ThemeMode::Custom,
        custom: CustomTheme {
            background: Some(ThemeColor(0x101827)),
            foreground: Some(ThemeColor(0xe8edf2)),
            accent_rgb: Some(ThemeColor(0x61aaff)),
            ..Default::default()
        },
        font_family: Some("DejaVu Sans Mono".into()),
        font_size: 16.,
        shell: ShellKind::Bash,
        ..Default::default()
    }
}

#[test]
fn round_trip_restores_theme_colors_and_font_by_name() {
    let store = MemoryStore::default();
    store.save(&preferences()).unwrap();
    let loaded = store.load().unwrap();
    let mut settings = Settings::default();
    loaded.apply(
        &mut settings,
        &["other".into(), "DejaVu Sans Mono".into()],
        false,
    );
    assert_eq!(settings.mode, ThemeMode::Custom);
    assert_eq!(settings.font_family, 1);
    assert_eq!(settings.font_size(), 16.);
    assert_eq!(settings.shell, ShellKind::Bash);
    assert_eq!(
        settings.palette_for(settings.mode, false).terminal,
        0x101827
    );
    assert_eq!(settings.palette_for(settings.mode, false).accent, 0x61aaff);
}

#[test]
fn invalid_input_is_rejected_without_rewriting_it() {
    for text in [
        "{",
        r##"{"custom":{"background":"#bad"}}"##,
        // 版本比当前新：不能降级，也不能用默认值覆盖用户文件。
        r#"{"version":99}"#,
        r#"{"font_size":900}"#,
    ] {
        let store = MemoryStore::with_contents(text);
        assert!(store.load().is_err(), "{text}");
        assert_eq!(store.contents(), Some(text.to_string()), "{text}");
    }
}

#[test]
fn missing_file_is_not_an_error() {
    assert!(MemoryStore::default().load().is_ok());
}

#[test]
fn oversized_files_are_rejected_before_parsing() {
    let store = MemoryStore::with_contents(" ".repeat((MAX_BYTES as usize) + 1));
    assert!(store.load().is_err());
}
