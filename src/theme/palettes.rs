// @Time    : 2026/11/4 23:30
// @Author  : fzf
// @FileName: palettes.rs
// @Software: Siskin

//! 内置配色。色值以官方色板为准（Catppuccin Mocha、Tokyo Night
//! 取自各自上游色板，见 `docs/themes.md`）；其余四套按同一套
//! WCAG 4.5:1 标尺校准过，`theme::tests` 里逐套断言。

use super::{Palette, ThemeId};

pub const PAPER: Palette = Palette {
    id: ThemeId::Paper,
    dark: false,
    canvas: 0xffffff,
    window: 0xffffff,
    sidebar: 0xffffff,
    terminal: 0xffffff,
    ink: 0x3b3d42,
    muted: 0x62666f,
    faint: 0x70757e,
    border: 0xcdcdcd,
    hairline: 0xe7e7e7,
    // 选区要比纯白底看得出深浅，但不抢正文。
    tint: 0xdde7f0,
    // 旧值与 ink 相同（0x3b3d42），光标与正文没有区别；改成深 slate 蓝，
    // 光标、活动标签下划线、焦点边框才有自己的颜色。
    accent: 0x2f5b8f,
    accent_soft: 0xc9d6e4,
    badge: 0xedf0f3,
    success: 0x557459,
    prompt: 0x87618d,
    command: 0x3b3d42,
    output: 0x3b3d42,
    system: 0x62666f,
    path: 0x496c9b,
    ok: 0x557459,
    // 旧值 0x8f703f 在白底上只有 4.6:1，余量太小，压深一档。
    warn: 0x7d5f31,
    error: 0x9e5962,
    link: 0x496c9b,
    faint_text: 0x70757e,
};

pub const GRAPHITE: Palette = Palette {
    id: ThemeId::Graphite,
    dark: true,
    canvas: 0x17191f,
    window: 0x1e222b,
    sidebar: 0x1b1e25,
    terminal: 0x14171d,
    ink: 0xe8ecf4,
    muted: 0xafb8c9,
    faint: 0x949fb2,
    border: 0x343b49,
    hairline: 0x272d38,
    tint: 0x2a324c,
    accent: 0xa6b4ff,
    accent_soft: 0x49577e,
    badge: 0x2b313d,
    success: 0x8fc9aa,
    prompt: 0xc5a5ef,
    command: 0xe8ecf4,
    output: 0xcbd3e1,
    system: 0x949fb2,
    path: 0xa6b4ff,
    ok: 0x8fc9aa,
    warn: 0xe5c181,
    error: 0xf09b9d,
    link: 0xacc4ff,
    faint_text: 0x949fb2,
};

pub const MOSS: Palette = Palette {
    id: ThemeId::Moss,
    dark: false,
    canvas: 0xdde5d9,
    window: 0xf4f8f0,
    sidebar: 0xe8efdf,
    terminal: 0xfbfdf7,
    ink: 0x222a22,
    // 旧 muted 0x66735f 只有 4.7:1，压深一档给 faint 留出梯度。
    muted: 0x5b6854,
    // 旧 faint 0x8d9a86 在窗口底上只有 2.8:1；取旧 muted 的值，仍弱于 muted。
    faint: 0x66735f,
    border: 0xcbd7c2,
    hairline: 0xdde7d5,
    tint: 0xdcebd3,
    accent: 0x2f6b3c,
    accent_soft: 0xc4d9b9,
    badge: 0xe3ecdb,
    success: 0x2f6b3c,
    prompt: 0x6a4f8f,
    command: 0x222a22,
    output: 0x3f4a3d,
    // 旧 system 0x76836f 在终端底上只有 3.9:1；与 muted 对齐。
    system: 0x5b6854,
    path: 0x2f6b3c,
    ok: 0x2f6b3c,
    warn: 0x8a6414,
    error: 0x9c3b34,
    link: 0x38628f,
    // 旧 faint_text 0x9aa691 在终端底上只有 2.5:1；与 faint 对齐。
    faint_text: 0x66735f,
};

pub const DUSK: Palette = Palette {
    id: ThemeId::Dusk,
    dark: true,
    canvas: 0x171a24,
    window: 0x1e222e,
    sidebar: 0x1a1e29,
    terminal: 0x161923,
    ink: 0xe4e6ef,
    muted: 0xa0a6b8,
    // 旧 faint 0x717892 在窗口底上只有 3.6:1；取 system 的值，仍弱于 muted。
    faint: 0x8b92a8,
    border: 0x2f3546,
    hairline: 0x252a38,
    tint: 0x27324a,
    accent: 0x7fa6e8,
    accent_soft: 0x33415e,
    badge: 0x272c3a,
    success: 0x7fc9a2,
    prompt: 0xc3a6ef,
    command: 0xf0f2fa,
    output: 0xc6cbdb,
    system: 0x8b92a8,
    path: 0x7fc9a2,
    ok: 0x7fc9a2,
    warn: 0xe0b96f,
    error: 0xe48b8b,
    link: 0x8db6f0,
    // 旧 faint_text 0x646b82 在终端底上只有 3.3:1；与 faint 对齐。
    faint_text: 0x8b92a8,
};

/// Catppuccin Mocha。色值取自上游色板（`docs/themes.md` 有来源）：
/// base/mantle/crust 作四级底色，text/subtext 作三级文字，
/// mauve 作强调色（也是光标色），surface 作选区与边框。
pub const CATPPUCCIN_MOCHA: Palette = Palette {
    id: ThemeId::CatppuccinMocha,
    dark: true,
    canvas: 0x11111b,
    window: 0x1e1e2e,
    sidebar: 0x181825,
    // 终端底用 mantle，比窗口底深一档，与其余深色主题的层级一致。
    terminal: 0x181825,
    ink: 0xcdd6f4,
    muted: 0xbac2de,
    faint: 0xa6adc8,
    border: 0x313244,
    hairline: 0x181825,
    tint: 0x313244,
    accent: 0xcba6f7,
    accent_soft: 0x45475a,
    badge: 0x313244,
    success: 0xa6e3a1,
    prompt: 0xf5c2e7,
    command: 0xcdd6f4,
    output: 0xbac2de,
    system: 0xa6adc8,
    path: 0x89b4fa,
    ok: 0xa6e3a1,
    warn: 0xf9e2af,
    error: 0xf38ba8,
    link: 0x89dceb,
    faint_text: 0xa6adc8,
};

/// Tokyo Night（Night 变体）。色值取自上游色板（`docs/themes.md` 有来源）：
/// bg/bg_dark 作底色，fg/fg_dark 作文字，blue 作强调色（也是光标色），
/// bg_visual（#283457）是本主题官方的选区色。
pub const TOKYO_NIGHT: Palette = Palette {
    id: ThemeId::TokyoNight,
    dark: true,
    canvas: 0x16161e,
    window: 0x1a1b26,
    sidebar: 0x16161e,
    terminal: 0x16161e,
    ink: 0xc0caf5,
    muted: 0xa9b1d6,
    // 官方 dark5 #737aa2 在窗口底上只有 4.1:1，未达 4.5，往 fg 方向提亮一档；
    // 仍弱于 muted（8.1:1），保留“弱化文字”的梯度。
    faint: 0x7f8bb3,
    border: 0x292e42,
    // 本主题的光标行底色，比窗口底略深一档。
    hairline: 0x232433,
    tint: 0x283457,
    accent: 0x7aa2f7,
    accent_soft: 0x292e42,
    badge: 0x292e42,
    success: 0x9ece6a,
    prompt: 0xbb9af7,
    command: 0xc0caf5,
    output: 0xa9b1d6,
    system: 0x7f8bb3,
    path: 0x7aa2f7,
    ok: 0x9ece6a,
    warn: 0xe0af68,
    error: 0xf7768e,
    link: 0x7dcfff,
    faint_text: 0x7f8bb3,
};
