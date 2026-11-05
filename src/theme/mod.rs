// @Time    : 2026/11/4 23:30
// @Author  : fzf
// @FileName: mod.rs
// @Software: Siskin

mod colors;
mod fonts;
mod palettes;
#[cfg(test)]
mod tests;

pub use colors::{ACCENTS, CustomTheme, ThemeColor};
pub use fonts::{mono_fonts, ui_family};
pub use palettes::{DUSK, GRAPHITE, MOSS, PAPER};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeId {
    Paper,
    Graphite,
    Moss,
    Dusk,
    Custom,
}

impl ThemeId {
    pub const PRESETS: [ThemeId; 4] = [
        ThemeId::Paper,
        ThemeId::Graphite,
        ThemeId::Moss,
        ThemeId::Dusk,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ThemeId::Paper => "Paper",
            ThemeId::Graphite => "Graphite",
            ThemeId::Moss => "Moss",
            ThemeId::Dusk => "Dusk",
            ThemeId::Custom => "自定义",
        }
    }

    pub fn detail(self) -> &'static str {
        match self {
            ThemeId::Paper => "暖白纸面",
            ThemeId::Graphite => "中性深灰",
            ThemeId::Moss => "苔绿浅色",
            ThemeId::Dusk => "靛蓝夜色",
            ThemeId::Custom => "自选底色与强调色",
        }
    }

    pub fn is_dark(self) -> bool {
        matches!(self, ThemeId::Graphite | ThemeId::Dusk)
    }
}

/// What the user picked. `System` resolves at runtime from the desktop.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    System,
    Light,
    Dark,
    Custom,
}

impl ThemeMode {
    pub const ALL: [ThemeMode; 4] = [
        ThemeMode::System,
        ThemeMode::Light,
        ThemeMode::Dark,
        ThemeMode::Custom,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ThemeMode::System => "跟随系统",
            ThemeMode::Light => "浅色",
            ThemeMode::Dark => "深色",
            ThemeMode::Custom => "自定义",
        }
    }

    pub fn next(self) -> ThemeMode {
        let index = Self::ALL.iter().position(|mode| *mode == self).unwrap_or(0);
        Self::ALL[(index + 1) % Self::ALL.len()]
    }
}

/// Resolves the mode against the desktop, falling back to Paper when the
/// platform has no opinion.
pub fn resolve(mode: ThemeMode, _custom: CustomTheme, system_dark: bool) -> ThemeId {
    match mode {
        ThemeMode::System => {
            if system_dark {
                ThemeId::Graphite
            } else {
                ThemeId::Paper
            }
        }
        ThemeMode::Light => ThemeId::Paper,
        ThemeMode::Dark => ThemeId::Graphite,
        ThemeMode::Custom => ThemeId::Custom,
    }
}

#[derive(Clone, Copy)]
pub struct Palette {
    pub id: ThemeId,
    pub dark: bool,
    pub canvas: u32,
    pub window: u32,
    pub sidebar: u32,
    pub terminal: u32,
    pub ink: u32,
    pub muted: u32,
    pub faint: u32,
    pub border: u32,
    pub hairline: u32,
    pub tint: u32,
    pub accent: u32,
    pub accent_soft: u32,
    pub badge: u32,
    pub success: u32,
    pub prompt: u32,
    pub command: u32,
    pub output: u32,
    pub system: u32,
    pub path: u32,
    pub ok: u32,
    pub warn: u32,
    pub error: u32,
    pub link: u32,
    pub faint_text: u32,
}

impl Palette {
    pub fn of(theme: ThemeId) -> Palette {
        match theme {
            ThemeId::Paper => PAPER,
            ThemeId::Graphite => GRAPHITE,
            ThemeId::Moss => MOSS,
            ThemeId::Dusk => DUSK,
            ThemeId::Custom => Self::custom(CustomTheme::default()),
        }
    }

    /// Builds a custom palette from a base tone and an accent. Dark bases get
    /// lifted text and darkened surfaces so contrast stays readable.
    pub fn custom(custom: CustomTheme) -> Palette {
        let accent = custom.accent_color();
        let mut palette = if custom.dark { GRAPHITE } else { PAPER };
        palette.id = ThemeId::Custom;
        palette.dark = custom.dark;
        palette.accent = accent;
        palette.path = accent;
        if let Some(color) = custom.background {
            palette.terminal = color.0;
        }
        if let Some(color) = custom.foreground {
            palette.ink = color.0;
            palette.command = color.0;
            palette.output = color.0;
        }
        palette.accent_soft = if custom.dark {
            mix(palette.window, accent, 0.40)
        } else {
            mix(palette.window, accent, 0.25)
        };
        palette.tint = if custom.dark {
            mix(palette.window, accent, 0.16)
        } else {
            mix(palette.window, accent, 0.10)
        };
        palette
    }

    /// Three colors that summarize the palette in a swatch.
    pub fn swatch(&self) -> [u32; 3] {
        [self.canvas, self.terminal, self.accent]
    }

    /// Status color used for the shell prompt marker.
    pub fn shell_color(&self) -> u32 {
        self.success
    }
}

/// Blends `a` toward `b`; `t` is how much of `b` to take.
fn mix(a: u32, b: u32, t: f32) -> u32 {
    let t = t.clamp(0., 1.);
    let channel = |shift: u32| {
        let left = ((a >> shift) & 0xff) as f32;
        let right = ((b >> shift) & 0xff) as f32;
        ((left * (1. - t) + right * t).round() as u32).min(0xff)
    };
    (channel(16) << 16) | (channel(8) << 8) | channel(0)
}
