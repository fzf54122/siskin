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
use crate::settings::{CustomTheme, FONT_SIZES, Settings, ThemeId, ThemeMode};
use crate::theme::glass::GLASS_OPACITY_DEFAULT;

/// 当前配置版本。迁移链从 1 开始，每加一版在这里和 `migrate` 里各加一步。
pub(crate) const CURRENT_VERSION: u32 = 2;
pub(crate) const MAX_BYTES: u64 = 65_536;

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Preferences {
    pub version: u32,
    pub theme_mode: ThemeMode,
    /// 命令面板点选的预设；`None` 为按 `theme_mode` 解析（旧行为）。
    /// 旧配置文件没有这个字段时按 `None` 处理，不用升版本。
    pub theme_preset: Option<ThemeId>,
    /// 玻璃效果开关。旧配置文件没有这个字段时默认开启（与当前默认
    /// 一致），所以用 `default_glass_enabled` 而不是 `bool` 的 `Default`。
    #[serde(default = "default_glass_enabled")]
    pub glass: bool,
    /// 浮层底色不透明度。缺字段时回默认；手改越界在 `apply` 里钳制，
    /// 不拒绝整份配置。
    #[serde(default = "default_glass_opacity")]
    pub glass_opacity: f32,
    pub custom: CustomTheme,
    pub font_family: Option<String>,
    pub font_size: f32,
    pub shell: ShellKind,
    /// 上次关闭时的会话快照，用于「恢复上次会话」。单条损坏时只丢
    /// 那一条（见 `de_lenient_sessions`），不让整份配置失效。
    #[serde(default, deserialize_with = "de_lenient_sessions")]
    pub sessions: Vec<SessionSnapshot>,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            version: CURRENT_VERSION,
            theme_mode: ThemeMode::System,
            theme_preset: None,
            glass: true,
            glass_opacity: GLASS_OPACITY_DEFAULT,
            custom: CustomTheme::default(),
            font_family: None,
            font_size: 13.,
            shell: ShellKind::Zsh,
            sessions: Vec::new(),
        }
    }
}

/// 快照数组的宽容反序列化：逐条解析，坏条目直接丢掉，好条目保留。
/// 快照是追加写的使用记录，一条被手改坏了不该连累主题、字体、shell
/// 选择与其它会话；整段根本不是数组时仍按错误返回，走整份配置的
/// 回退路径（`load_preferences` 用默认值启动并保留原文件）。
fn de_lenient_sessions<'de, D>(deserializer: D) -> Result<Vec<SessionSnapshot>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let values = <Vec<serde_json::Value> as serde::Deserialize>::deserialize(deserializer)?;
    Ok(values
        .into_iter()
        .filter_map(|value| serde_json::from_value::<SessionSnapshot>(value).ok())
        .collect())
}

impl Preferences {
    pub(crate) fn from_settings(settings: &Settings, fonts: &[String]) -> Self {
        Self {
            theme_mode: settings.mode,
            theme_preset: settings.preset,
            glass: settings.glass,
            glass_opacity: settings.glass_opacity,
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
        settings.glass = self.glass;
        // 手改的透度可能越界或非有限，在这里钳制，不拒绝整份配置。
        settings.set_glass_opacity(self.glass_opacity);
        settings.set_mode(self.theme_mode, system_dark);
        // `Custom` 不是预设：手改的配置文件里出现时直接丢弃，不破坏解析。
        settings.preset = self.theme_preset.filter(|theme| *theme != ThemeId::Custom);
        settings.resolve_theme(system_dark);
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

fn default_glass_enabled() -> bool {
    true
}

fn default_glass_opacity() -> f32 {
    GLASS_OPACITY_DEFAULT
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picked_preset_survives_a_save_and_load_round_trip() {
        let mut settings = Settings::default();
        settings.set_theme(ThemeId::TokyoNight, false);
        let stored = Preferences::from_settings(&settings, &[]);
        let json = serde_json::to_string(&stored).expect("配置必须能序列化");
        let loaded: Preferences = serde_json::from_str(&json).expect("配置必须能反序列化");
        assert_eq!(loaded.theme_preset, Some(ThemeId::TokyoNight));

        let mut restored = Settings::default();
        loaded.apply(&mut restored, &[], false);
        assert_eq!(restored.theme, ThemeId::TokyoNight);
        assert_eq!(restored.preset, Some(ThemeId::TokyoNight));
    }

    #[test]
    fn old_config_without_a_preset_field_still_loads() {
        // 升级前的配置文件没有 theme_preset 字段：按“按模式解析”处理，
        // 不能因此拒绝整份配置。
        let loaded: Preferences = serde_json::from_str(r#"{"version":2,"theme_mode":"dark"}"#)
            .expect("旧配置必须能反序列化");
        assert_eq!(loaded.theme_preset, None);
        let mut settings = Settings::default();
        loaded.apply(&mut settings, &[], false);
        assert_eq!(settings.theme, ThemeId::Graphite);
    }

    #[test]
    fn a_hand_edited_custom_preset_is_dropped_not_fatal() {
        let loaded: Preferences =
            serde_json::from_str(r#"{"version":2,"theme_mode":"light","theme_preset":"custom"}"#)
                .expect("配置必须能反序列化");
        let mut settings = Settings::default();
        loaded.apply(&mut settings, &[], false);
        assert_eq!(settings.preset, None);
        assert_eq!(settings.theme, ThemeId::Paper);
    }

    #[test]
    fn glass_setting_survives_a_save_and_load_round_trip() {
        let mut settings = Settings::default();
        settings.toggle_glass();
        settings.set_glass_opacity(0.95);
        let stored = Preferences::from_settings(&settings, &[]);
        let json = serde_json::to_string(&stored).expect("配置必须能序列化");
        let loaded: Preferences = serde_json::from_str(&json).expect("配置必须能反序列化");
        assert!(!loaded.glass);
        assert_eq!(loaded.glass_opacity, 0.95);

        let mut restored = Settings::default();
        loaded.apply(&mut restored, &[], false);
        assert!(!restored.glass);
        assert_eq!(restored.glass_opacity, 0.95);
    }

    #[test]
    fn old_config_without_glass_fields_loads_with_glass_on() {
        // 升级前的配置文件没有 glass 字段：默认开启、透度 0.85，
        // 不能因此拒绝整份配置。
        let loaded: Preferences = serde_json::from_str(r#"{"version":2,"theme_mode":"dark"}"#)
            .expect("旧配置必须能反序列化");
        assert!(loaded.glass);
        assert_eq!(loaded.glass_opacity, GLASS_OPACITY_DEFAULT);
        let mut settings = Settings::default();
        settings.toggle_glass();
        loaded.apply(&mut settings, &[], false);
        assert!(settings.glass);
        assert_eq!(settings.glass_opacity, GLASS_OPACITY_DEFAULT);
    }

    #[test]
    fn a_hand_edited_opacity_is_clamped_not_fatal() {
        let loaded: Preferences = serde_json::from_str(r#"{"version":2,"glass_opacity":0.2}"#)
            .expect("配置必须能反序列化");
        let mut settings = Settings::default();
        loaded.apply(&mut settings, &[], false);
        assert_eq!(settings.glass_opacity, GLASS_OPACITY_DEFAULT);
        let loaded: Preferences = serde_json::from_str(r#"{"version":2,"glass_opacity":1.5}"#)
            .expect("配置必须能反序列化");
        loaded.apply(&mut settings, &[], false);
        assert_eq!(settings.glass_opacity, 0.95);
    }
}
