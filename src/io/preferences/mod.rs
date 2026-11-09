// @Time    : 2026/11/5 21:10
// @Author  : fzf
// @FileName: mod.rs
// @Software: Siskin

//! 版本化用户配置。字体按名字存，换机器不会串位；写入是原子替换，
//! 损坏的文件只告警，不会被默认值覆盖。
//!
//! 版本变化通过 `migrate` 责任链逐级处理，读取时先迁移再反序列化。

mod migrate;
mod session_state;
mod store;

pub(crate) use migrate::migrate;
pub(crate) use session_state::SessionSnapshot;
pub(crate) use store::{FileStore, Store};

#[cfg(test)]
pub(crate) use store::MemoryStore;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::domain::ShellKind;
use crate::settings::{CustomTheme, FONT_SIZES, Settings, ThemeMode};

/// 当前配置版本。迁移链从 1 开始，每加一版在这里和 `migrate` 里各加一步。
pub(crate) const CURRENT_VERSION: u32 = 2;
pub(crate) const MAX_BYTES: u64 = 65_536;

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Preferences {
    pub version: u32,
    pub theme_mode: ThemeMode,
    pub custom: CustomTheme,
    pub font_family: Option<String>,
    pub font_size: f32,
    pub shell: ShellKind,
    /// 上次关闭时的会话快照，用于「恢复上次会话」。
    pub sessions: Vec<SessionSnapshot>,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            version: CURRENT_VERSION,
            theme_mode: ThemeMode::System,
            custom: CustomTheme::default(),
            font_family: None,
            font_size: 13.,
            shell: ShellKind::Zsh,
            sessions: Vec::new(),
        }
    }
}

impl Preferences {
    pub(crate) fn from_settings(settings: &Settings, fonts: &[String]) -> Self {
        Self {
            theme_mode: settings.mode,
            custom: settings.custom,
            font_family: fonts.get(settings.font_family).cloned(),
            font_size: settings.font_size(),
            shell: settings.shell,
            ..Self::default()
        }
    }

    pub(crate) fn apply(&self, settings: &mut Settings, fonts: &[String], system_dark: bool) {
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

    /// 校验解码后的数据。存储层只负责迁移，字段约束都在这里。
    fn validate(self) -> Result<Self, String> {
        if self.version != CURRENT_VERSION {
            return Err(format!(
                "不支持的设置版本 {}（当前 {CURRENT_VERSION}）",
                self.version
            ));
        }
        if !FONT_SIZES.contains(&self.font_size) {
            return Err(format!("不支持的字号 {}", self.font_size));
        }
        Ok(self)
    }
}

pub(crate) fn default_path() -> Option<PathBuf> {
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
