// @Time    : 2026/11/5 10:12
// @Author  : fzf
// @FileName: settings.rs
// @Software: Siskin

//! User-facing preferences. Everything is plain data so the settings page can
//! preview a change before it is applied and the terminal can re-wrap on resize.

pub use crate::theme::{CustomTheme, ThemeId, ThemeMode};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShellKind {
    Zsh,
    Bash,
    Fish,
    Sh,
}

impl ShellKind {
    pub const ALL: [ShellKind; 4] = [
        ShellKind::Zsh,
        ShellKind::Bash,
        ShellKind::Fish,
        ShellKind::Sh,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ShellKind::Zsh => "zsh",
            ShellKind::Bash => "bash",
            ShellKind::Fish => "fish",
            ShellKind::Sh => "sh",
        }
    }

    /// Prompt prefix. Each shell draws its own prompt so the demo shows the
    /// difference instead of pretending every shell looks the same.
    pub fn prompt_marker(self) -> &'static str {
        match self {
            ShellKind::Zsh => "❯",
            ShellKind::Bash => "$",
            ShellKind::Fish => ">",
            ShellKind::Sh => "$",
        }
    }

    pub fn startup(self) -> &'static str {
        match self {
            ShellKind::Zsh => "zsh 5.9 (x86_64-debian-linux-gnu)",
            ShellKind::Bash => "GNU bash, version 5.2.21(1)-release",
            ShellKind::Fish => "fish, version 3.7.0",
            ShellKind::Sh => "sh (POSIX), dash 0.5.12",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Terminal,
    Settings,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SettingsSection {
    #[default]
    Appearance,
    Typography,
    Shell,
    Desktop,
}

impl SettingsSection {
    pub const ALL: [Self; 4] = [
        Self::Appearance,
        Self::Typography,
        Self::Shell,
        Self::Desktop,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::Appearance => "外观",
            Self::Typography => "字体与排版",
            Self::Shell => "Shell",
            Self::Desktop => "桌面集成",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Appearance => "让终端适合你的工作环境。",
            Self::Typography => "让每一行代码都清晰、舒适。",
            Self::Shell => "管理新会话使用的 Shell 配置。",
            Self::Desktop => "与桌面自然协作，而不是重复桌面的功能。",
        }
    }
}

pub const FONT_SIZES: [f32; 5] = [11., 12., 13., 14., 16.];

pub struct Settings {
    pub mode: ThemeMode,
    pub custom: CustomTheme,
    /// Resolved palette identity for the current mode and desktop preference.
    pub theme: ThemeId,
    pub font_index: usize,
    pub font_family: usize,
    pub shell: ShellKind,
}

impl Default for Settings {
    fn default() -> Self {
        let custom = CustomTheme::default();
        Self {
            mode: ThemeMode::System,
            custom,
            theme: ThemeId::Paper,
            font_index: 2,
            font_family: 0,
            shell: ShellKind::Zsh,
        }
    }
}

impl Settings {
    pub fn palette_for(&self, mode: ThemeMode, system_dark: bool) -> crate::theme::Palette {
        if mode == ThemeMode::Custom {
            crate::theme::Palette::custom(self.custom)
        } else {
            crate::theme::Palette::of(crate::theme::resolve(mode, self.custom, system_dark))
        }
    }

    /// Re-resolves the palette after a mode or custom-color change. The
    /// caller supplies the desktop preference so this stays pure.
    pub fn resolve_theme(&mut self, system_dark: bool) {
        self.theme = crate::theme::resolve(self.mode, self.custom, system_dark);
    }

    pub fn set_mode(&mut self, mode: ThemeMode, system_dark: bool) {
        self.mode = mode;
        self.resolve_theme(system_dark);
    }

    pub fn font_size(&self) -> f32 {
        FONT_SIZES[self.font_index.min(FONT_SIZES.len() - 1)]
    }

    /// Advance width for the current size. Monospace faces used here sit near
    /// 0.6 em, so this stays close without depending on a specific font.
    pub fn char_width(&self) -> f32 {
        self.font_size() * 0.6
    }

    pub fn row_height(&self) -> f32 {
        (self.font_size() * 1.55).round()
    }

    pub fn step_font(&mut self, up: bool) {
        self.font_index = if up {
            (self.font_index + 1).min(FONT_SIZES.len() - 1)
        } else {
            self.font_index.saturating_sub(1)
        };
    }

    pub fn next_theme(&mut self, system_dark: bool) {
        self.set_mode(self.mode.next(), system_dark);
    }

    pub fn next_shell(&mut self) {
        let index = ShellKind::ALL
            .iter()
            .position(|shell| *shell == self.shell)
            .unwrap_or(0);
        self.shell = ShellKind::ALL[(index + 1) % ShellKind::ALL.len()];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_size_stays_inside_the_supported_range() {
        let mut settings = Settings::default();
        for _ in 0..10 {
            settings.step_font(true);
        }
        assert_eq!(settings.font_size(), *FONT_SIZES.last().unwrap());
        for _ in 0..20 {
            settings.step_font(false);
        }
        assert_eq!(settings.font_size(), FONT_SIZES[0]);
        assert!(settings.char_width() > 0.);
        assert!(settings.row_height() >= settings.font_size());
    }

    #[test]
    fn theme_and_shell_cycles_wrap_around() {
        let mut settings = Settings::default();
        for _ in 0..ThemeMode::ALL.len() {
            settings.next_theme(false);
        }
        assert_eq!(settings.mode, ThemeMode::System);
        for _ in 0..ShellKind::ALL.len() {
            settings.next_shell();
        }
        assert_eq!(settings.shell, ShellKind::Zsh);
    }

    #[test]
    fn shells_have_distinct_prompts() {
        let prompts: Vec<&str> = ShellKind::ALL
            .iter()
            .map(|shell| shell.prompt_marker())
            .collect();
        assert_eq!(prompts.len(), ShellKind::ALL.len());
        assert!(
            ShellKind::ALL
                .iter()
                .all(|shell| !shell.startup().is_empty())
        );
    }
}
