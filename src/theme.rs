// @Time    : 2026/11/4 19:42
// @Author  : fzf
// @FileName: theme.rs
// @Software: Siskin

//! Siskin's light palette. Colors carry meaning: one accent, one warm signal,
//! and quiet greys for everything that is only structure.

pub const CANVAS: u32 = 0xe9ebe8;
pub const WINDOW: u32 = 0xfbfbfa;
pub const SIDEBAR: u32 = 0xf3f4f2;
pub const TERMINAL: u32 = 0xfdfdfc;
pub const INK: u32 = 0x26282b;
pub const MUTED: u32 = 0x73787d;
pub const FAINT: u32 = 0x9aa0a4;
pub const BORDER: u32 = 0xdfe2de;
pub const HAIRLINE: u32 = 0xebece8;
pub const TINT: u32 = 0xeaf0e6;
pub const ACCENT: u32 = 0x3c6e53;
pub const SUCCESS: u32 = 0x3c7a58;
pub const BADGE: u32 = 0xeceee9;

/// Terminal text roles, kept close to the light theme instead of a neon ANSI set.
pub const PROMPT: u32 = 0x7c5aa6;
pub const COMMAND: u32 = 0x26282b;
pub const OUTPUT: u32 = 0x4a4f54;
pub const SYSTEM: u32 = 0x8a9096;
pub const PATH: u32 = 0x3c6e53;
pub const OK: u32 = 0x3c7a58;
pub const WARN: u32 = 0x9a6a1e;
pub const ERROR: u32 = 0xa6473f;
pub const LINK: u32 = 0x3f6ea8;
pub const FAINT_TEXT: u32 = 0xa8adaf;

pub const MONO_FONTS: [&str; 5] = [
    "JetBrains Mono",
    "MesloLGL Nerd Font Mono",
    "DejaVu Sans Mono",
    "Noto Sans Mono",
    "Liberation Mono",
];

/// Pick the first installed monospace family so the layout looks the same on
/// machines without the preferred font.
pub fn mono_family(installed: &[String]) -> &'static str {
    MONO_FONTS
        .iter()
        .copied()
        .find(|candidate| installed.iter().any(|name| name == candidate))
        .unwrap_or("monospace")
}
