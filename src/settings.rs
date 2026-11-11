// @Time    : 2026/11/5 10:12
// @Author  : fzf
// @FileName: settings.rs
// @Software: Siskin

//! User-facing preferences. Everything is plain data so the settings page can
//! preview a change before it is applied and the terminal can re-wrap on resize.

pub(crate) use crate::domain::ShellKind;
pub use crate::theme::{CustomTheme, ThemeId, ThemeMode};

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
            Self::Shell => "Shell 与会话",
            Self::Desktop => "桌面集成",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Appearance => "主题、颜色与界面外观。",
            Self::Typography => "终端字体、字号与字形预览。",
            Self::Shell => "管理新会话使用的 Shell 配置。",
            Self::Desktop => "全局菜单与本地配置。",
        }
    }
}

pub const FONT_SIZES: [f32; 5] = [11., 12., 13., 14., 16.];

use crate::theme::glass::{
    GLASS_OPACITY_DEFAULT, clamp_opacity, fill_rgba, next_opacity, surface_rgba,
    terminal_fill_rgba, terminal_opacity,
};

pub struct Settings {
    pub mode: ThemeMode,
    pub custom: CustomTheme,
    /// Resolved palette identity for the current mode and desktop preference.
    pub theme: ThemeId,
    /// A preset picked directly (command palette). It wins over `mode` until
    /// the user picks a mode again, and it is persisted with the settings.
    /// `None` means "resolve from `mode`", the historical behavior.
    pub preset: Option<ThemeId>,
    pub font_index: usize,
    pub font_family: usize,
    pub shell: ShellKind,
    /// 磨砂玻璃开关，默认开启：整窗玻璃——终端主区底色也透出
    /// 桌面（下限 0.92），浮层与 chrome 按透度档半透明。
    pub glass: bool,
    /// 浮层底色不透明度，恒在 `GLASS_OPACITY_MIN..=MAX` 内；
    /// 下限 0.85 是六套预设逐套验算后能保住 4.5:1 对比度的最低值。
    pub glass_opacity: f32,
}

impl Default for Settings {
    fn default() -> Self {
        let custom = CustomTheme::default();
        Self {
            mode: ThemeMode::System,
            custom,
            theme: ThemeId::Paper,
            preset: None,
            font_index: 2,
            font_family: 0,
            shell: ShellKind::Zsh,
            glass: true,
            glass_opacity: GLASS_OPACITY_DEFAULT,
        }
    }
}

impl Settings {
    pub fn palette_for(&self, mode: ThemeMode, system_dark: bool) -> crate::theme::Palette {
        if mode == ThemeMode::Custom {
            crate::theme::Palette::custom(self.custom)
        } else if let Some(preset) = self.preset {
            crate::theme::Palette::of(preset)
        } else {
            crate::theme::Palette::of(crate::theme::resolve(mode, self.custom, system_dark))
        }
    }

    /// Re-resolves the palette after a mode or custom-color change. The
    /// caller supplies the desktop preference so this stays pure.
    pub fn resolve_theme(&mut self, system_dark: bool) {
        self.theme = self.palette_for(self.mode, system_dark).id;
    }

    pub fn set_mode(&mut self, mode: ThemeMode, system_dark: bool) {
        self.mode = mode;
        // 换模式就是放弃之前点选的预设，回到按模式解析。
        self.preset = None;
        self.resolve_theme(system_dark);
    }

    /// Pins one preset. The mode follows its lightness so the settings page
    /// still shows a consistent light/dark choice; `Custom` is not a preset
    /// and is ignored here.
    pub fn set_theme(&mut self, theme: ThemeId, system_dark: bool) {
        if theme == ThemeId::Custom {
            return;
        }
        self.preset = Some(theme);
        self.mode = if theme.is_dark() {
            ThemeMode::Dark
        } else {
            ThemeMode::Light
        };
        self.resolve_theme(system_dark);
    }

    pub fn font_size(&self) -> f32 {
        FONT_SIZES[self.font_index.min(FONT_SIZES.len() - 1)]
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

    /// 玻璃开关：关掉即所有浮层回到实色。终端底玻璃也随开关走，
    /// 关掉即实色终端底；开着时终端底按终端下限透度透出桌面。
    pub fn toggle_glass(&mut self) {
        self.glass = !self.glass;
    }

    /// 透度三档循环（0.85 → 0.90 → 0.95 → 0.85）。
    pub fn cycle_glass_opacity(&mut self) {
        self.glass_opacity = next_opacity(self.glass_opacity);
    }

    /// 手改的配置值在这里钳制，不拒绝整份配置。
    pub fn set_glass_opacity(&mut self, value: f32) {
        self.glass_opacity = clamp_opacity(value);
    }

    /// 玻璃填充色（`0xRRGGBBAA`，直接喂 `gpui::rgba`）；关掉玻璃时
    /// alpha 恒为 0xFF，与原来的实色底完全一致。整窗画布与 tab
    /// 条之外的 chrome 用这个，不带磨砂 tint。
    pub fn glass_fill(&self, color: u32) -> u32 {
        fill_rgba(self.glass, self.glass_opacity, color)
    }

    /// 浮层玻璃填充色：深色主题先磨砂 tint 再给 alpha；关掉玻璃
    /// 时回到实色底。命令面板、搜索条、侧栏、弹窗用这个。
    pub fn glass_surface(&self, color: u32, dark: bool) -> u32 {
        surface_rgba(self.glass, self.glass_opacity, color, dark)
    }

    /// 终端底实际不透明度：玻璃关掉为 1.0（实色），开着时不低于
    /// 终端下限 0.92（`theme::glass::terminal_opacity`）。
    pub fn terminal_bg_alpha(&self) -> f32 {
        if self.glass {
            terminal_opacity(self.glass_opacity)
        } else {
            1.
        }
    }

    /// 终端底玻璃填充色（`0xRRGGBBAA`）：终端卡片的 div 底用这个，
    /// 网格自身的底色矩形另用 `terminal_bg_alpha`（见 grid）。
    pub fn glass_terminal_fill(&self, color: u32) -> u32 {
        terminal_fill_rgba(self.glass, self.glass_opacity, color)
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
        assert!(settings.row_height() >= settings.font_size());
    }

    #[test]
    fn theme_and_shell_cycles_wrap_around() {
        let mut settings = Settings::default();
        for _ in 0..ThemeMode::ALL.len() {
            settings.next_theme(false);
        }
        assert_eq!(settings.mode, ThemeMode::System);
    }

    #[test]
    fn picked_preset_wins_over_mode_until_mode_changes() {
        let mut settings = Settings::default();
        settings.set_theme(ThemeId::TokyoNight, false);
        assert_eq!(settings.theme, ThemeId::TokyoNight);
        assert_eq!(settings.mode, ThemeMode::Dark);
        // 跟随系统也不应把点选的预设顶掉。
        settings.resolve_theme(true);
        assert_eq!(settings.theme, ThemeId::TokyoNight);
        assert_eq!(
            settings.palette_for(settings.mode, false).id,
            ThemeId::TokyoNight
        );
        // 换模式即放弃预设，回到按模式解析。
        settings.set_mode(ThemeMode::Light, false);
        assert_eq!(settings.theme, ThemeId::Paper);
        assert_eq!(settings.preset, None);
    }

    #[test]
    fn picked_light_preset_reports_light_mode() {
        let mut settings = Settings::default();
        settings.set_theme(ThemeId::Moss, true);
        assert_eq!(settings.theme, ThemeId::Moss);
        assert_eq!(settings.mode, ThemeMode::Light);
    }

    #[test]
    fn custom_is_not_a_pinnable_preset() {
        let mut settings = Settings::default();
        settings.set_theme(ThemeId::Custom, false);
        assert_eq!(settings.preset, None);
        assert_eq!(settings.theme, ThemeId::Paper);
    }

    #[test]
    fn glass_defaults_to_on_at_the_most_transparent_safe_opacity() {
        let settings = Settings::default();
        assert!(settings.glass);
        assert_eq!(settings.glass_opacity, GLASS_OPACITY_DEFAULT);
        // 开着时填充带透明通道，关掉即实色。
        assert_eq!(
            settings.glass_fill(0x1e222b),
            0x1e_22_2b_d9,
            "默认透度 0.85 应为 alpha 217"
        );
        let mut settings = Settings::default();
        settings.toggle_glass();
        assert!(!settings.glass);
        assert_eq!(settings.glass_fill(0x1e222b), 0x1e_22_2b_ff);
    }

    #[test]
    fn glass_terminal_fill_floors_at_0_92_and_is_opaque_when_disabled() {
        let settings = Settings::default();
        // 默认档 0.85，终端底抬到 0.92：alpha 0xEB。
        assert_eq!(settings.terminal_bg_alpha(), 0.92);
        assert_eq!(settings.glass_terminal_fill(0x14171d), 0x14_17_1d_eb);
        let mut settings = Settings::default();
        settings.set_glass_opacity(0.95);
        assert_eq!(settings.terminal_bg_alpha(), 0.95);
        settings.toggle_glass();
        assert_eq!(settings.terminal_bg_alpha(), 1.);
        assert_eq!(settings.glass_terminal_fill(0x14171d), 0x14_17_1d_ff);
    }

    #[test]
    fn glass_surface_tints_dark_themes_only() {
        let settings = Settings::default();
        assert_eq!(settings.glass_surface(0x1e222b, true), 0x35_38_40_d9);
        assert_eq!(settings.glass_surface(0xffffff, false), 0xff_ff_ff_d9);
    }

    #[test]
    fn glass_opacity_cycles_and_clamps() {
        let mut settings = Settings::default();
        settings.cycle_glass_opacity();
        assert_eq!(settings.glass_opacity, 0.90);
        settings.cycle_glass_opacity();
        assert_eq!(settings.glass_opacity, 0.95);
        settings.cycle_glass_opacity();
        assert_eq!(settings.glass_opacity, GLASS_OPACITY_DEFAULT);
        settings.set_glass_opacity(2.);
        assert_eq!(settings.glass_opacity, 0.95);
        settings.set_glass_opacity(0.);
        assert_eq!(settings.glass_opacity, GLASS_OPACITY_DEFAULT);
        settings.set_glass_opacity(f32::NAN);
        assert_eq!(settings.glass_opacity, GLASS_OPACITY_DEFAULT);
    }
}
