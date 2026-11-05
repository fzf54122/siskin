// @Time    : 2026/11/4 23:05
// @Author  : fzf
// @FileName: preferences.rs
// @Software: Siskin

//! Versioned user preferences. Font names, not machine-local list indexes,
//! survive a restart. Atomic replacement never leaves a half-written file.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::settings::{CustomTheme, FONT_SIZES, Settings, ShellKind, ThemeMode};

#[derive(Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Preferences {
    pub version: u32,
    pub theme_mode: ThemeMode,
    pub custom: CustomTheme,
    pub font_family: Option<String>,
    pub font_size: f32,
    pub shell: ShellKind,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            version: 1,
            theme_mode: ThemeMode::System,
            custom: CustomTheme::default(),
            font_family: None,
            font_size: 13.,
            shell: ShellKind::Zsh,
        }
    }
}

impl Preferences {
    pub fn from_settings(settings: &Settings, fonts: &[String]) -> Self {
        Self {
            theme_mode: settings.mode,
            custom: settings.custom,
            font_family: fonts.get(settings.font_family).cloned(),
            font_size: settings.font_size(),
            shell: settings.shell,
            ..Self::default()
        }
    }

    pub fn apply(&self, settings: &mut Settings, fonts: &[String], system_dark: bool) {
        settings.custom = self.custom;
        settings.font_family = self
            .font_family
            .as_ref()
            .and_then(|name| fonts.iter().position(|font| font == name))
            .unwrap_or(0);
        settings.font_index = FONT_SIZES
            .iter()
            .position(|size| *size == self.font_size)
            .unwrap_or(2);
        settings.shell = self.shell;
        settings.set_mode(self.theme_mode, system_dark);
    }
}

pub fn default_path() -> Option<PathBuf> {
    if let Some(base) = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
    {
        return Some(base.join("siskin/settings.json"));
    }
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join(".config/siskin/settings.json"))
}

pub fn load(path: &Path) -> Result<Preferences, Box<dyn std::error::Error>> {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Preferences::default());
        }
        Err(error) => return Err(error.into()),
    };
    let mut bytes = Vec::new();
    file.take(65_537).read_to_end(&mut bytes)?;
    if bytes.len() > 65_536 {
        return Err("设置文件超过 64 KiB".into());
    }
    let prefs: Preferences = serde_json::from_slice(&bytes)?;
    if prefs.version != 1 || !FONT_SIZES.contains(&prefs.font_size) {
        return Err("不支持的设置版本或字号".into());
    }
    Ok(prefs)
}

pub fn save(path: &Path, prefs: &Preferences) -> Result<(), Box<dyn std::error::Error>> {
    let parent = path.parent().ok_or("设置路径没有父目录")?;
    std::fs::create_dir_all(parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    serde_json::to_writer_pretty(&mut temporary, prefs)?;
    temporary.write_all(b"\n")?;
    temporary.as_file().sync_all()?;
    temporary.persist(path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::ThemeColor;

    #[test]
    fn round_trip_restores_theme_colors_and_font_by_name() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("siskin/settings.json");
        let prefs = Preferences {
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
        };
        save(&path, &prefs).unwrap();
        let loaded = load(&path).unwrap();
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
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("settings.json");
        for text in [
            "{",
            r##"{"custom":{"background":"#bad"}} "##,
            r#"{"version":2}"#,
            r#"{"font_size":900}"#,
        ] {
            std::fs::write(&path, text).unwrap();
            assert!(load(&path).is_err());
            assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
        }
    }
}
