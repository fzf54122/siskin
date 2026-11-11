// @Time    : 2026/11/11
// @Author  : fzf
// @FileName: glass.rs
// @Software: Siskin

//! 磨砂玻璃：浮层、侧栏与终端主区的半透明底色。
//!
//! 核实结论（全文见 `docs/glass.md`）：
//! - GPUI 没有逐元素的 backdrop blur，只有窗口级
//!   `WindowBackgroundAppearance::Blurred`；
//! - Wayland 下它走 KDE 的 `org_kde_kwin_blur` 协议，只在 KWin
//!   实现，GNOME/Mutter 上静默退化为普通透明；
//! - X11 下 `Blurred` 与 `Transparent` 等价，只开透明不请求模糊；
//! - pulse-ui v0.1.1 没有任何 blur / backdrop / glass 接口。
//!
//! 所以玻璃是「半透明填充 + 磨砂 tint + 亮边/顶光/阴影」的近似，只
//! 在 KDE Plasma Wayland 上能叠加一道真模糊。整窗玻璃：终端主区
//! 也参与，底色透出桌面壁纸（Ghostty 的 background-opacity 那
//! 种做法），不再是只有浮层透、主区实底。
//!
//! 透度下限不是拍脑袋的：对六套预设逐套验算，0.75 时 Paper 的浮层
//! 文字只有 4.34:1、Moss 的侧栏次要文字只有 4.30:1，都跌破 WCAG
//! 4.5:1；0.80 时 Paper 只剩 4.50 零余量；0.85 起最差组合 4.67:1
//! 才有余量。因此浮层下限就是 0.85，三档是 0.85 / 0.90 / 0.95。
//!
//! 终端底另有更高的下限 0.92：终端文字直接盖在未知壁纸上，浅色
//! 主题的次要文字（`muted`）在 0.85 黑桌面下只有 4.08:1，0.90
//! 时 4.61 余量仍薄，0.92 起最差组合（Paper 的 `muted` 盖黑桌
//! 面）为 4.83:1。0.85/0.90 两档的终端底都按 0.92 画，浮层仍
//! 按所选档画；叠加最坏情况（浮层 0.85 + 整屏遮罩 + 终端底
//! 0.92 + 纯黑桌面）Paper 的报错文字为 4.58:1，仍在 4.5 之上。
//!
//! 磨砂 tint 只做在深色主题上：深色浮层底往白提 10%，肉眼可见
//! 又不伤对比度（最差组合仍在 5.0:1 之上）；浅色主题不压暗，
//! Paper 的浮层报错文字在 0.85 档本来就只有 4.58:1，任何压暗
//! 都会跌破 4.5:1，浅色玻璃的“边”靠亮边、顶光与阴影来读。

#[cfg(test)]
use super::colors::{contrast, mix};

/// 浮层底色不透明度下限。低于它，浅色主题在深色桌面/遮罩下的文字
/// 对比度会跌破 4.5:1（见模块头注释的验算）。
pub const GLASS_OPACITY_MIN: f32 = 0.85;

/// 浮层底色不透明度上限。再浓就和实色没有区别。
pub const GLASS_OPACITY_MAX: f32 = 0.95;

/// 终端底色不透明度下限，高于浮层下限：终端文字直接盖在未知
/// 壁纸上，`muted` 在 0.85 黑桌面下只有 4.08:1（见模块头）。
pub const GLASS_TERMINAL_OPACITY_MIN: f32 = 0.92;

/// 深色浮层底往白提的幅度：0x1e222b 提后约 0x353840，肉眼可见
/// 的磨砂感，又不伤对比度（单测里逐套断言）。
pub const GLASS_TINT_DARK: f32 = 0.10;

/// 浮层顶部高光线：白色 35% 透度，1px，画在浮层填充之上。
pub const GLASS_HIGHLIGHT_RGBA: u32 = 0xffff_ff59;

/// 默认透度：最通透且仍满足对比度的一档。
pub const GLASS_OPACITY_DEFAULT: f32 = 0.85;

/// 透度三档（值，设置页显示名）。默认取第一档。
pub const GLASS_OPACITY_STEPS: [(f32, &str); 3] = [(0.85, "标准"), (0.90, "较浓"), (0.95, "浓郁")];

/// 命令面板的整屏遮罩色（`ui/palette.rs`），`0xRRGGBBAA`。
/// 浮层文字的对比度断言按「这层遮罩压暗后的终端底」验算；
/// 关闭确认弹窗用的是另一层更淡的黑遮罩（`ui/close.rs`），
/// 压暗程度不如这层，断言取更严的这一层即覆盖两者。
pub const GLASS_SCRIM_RGBA: u32 = 0x0b10_2055;

/// 把越界透度钳回 `[MIN, MAX]`；非有限值（手改配置的 NaN/Inf）
/// 直接回默认，不让整份配置失效。
pub fn clamp_opacity(value: f32) -> f32 {
    if !value.is_finite() {
        return GLASS_OPACITY_DEFAULT;
    }
    value.clamp(GLASS_OPACITY_MIN, GLASS_OPACITY_MAX)
}

/// 透度循环：取比当前大的下一档，没有就回到第一档；
/// 手改的档外值按「就近向上」进档，不是静默回到默认。
pub fn next_opacity(current: f32) -> f32 {
    for (value, _) in GLASS_OPACITY_STEPS {
        if value > current + f32::EPSILON {
            return value;
        }
    }
    GLASS_OPACITY_STEPS[0].0
}

/// 离 `opacity` 最近的档位下标，供设置页判断哪个按钮高亮。
pub fn nearest_step(opacity: f32) -> usize {
    let mut best = 0;
    for (index, (value, _)) in GLASS_OPACITY_STEPS.iter().enumerate() {
        if (value - opacity).abs() < (GLASS_OPACITY_STEPS[best].0 - opacity).abs() {
            best = index;
        }
    }
    best
}

/// 终端底实际用的不透明度：浮层档位再通透，终端底也不低于
/// `GLASS_TERMINAL_OPACITY_MIN`。玻璃关闭时调用方直接用 1.0，
/// 这里只管开启时的下限。
pub fn terminal_opacity(opacity: f32) -> f32 {
    clamp_opacity(opacity).max(GLASS_TERMINAL_OPACITY_MIN)
}

/// 磨砂 tint：深色底往白提 `GLASS_TINT_DARK`，浅色底原样返回
/// （见模块头：浅色再压暗就跌破 4.5:1）。
pub fn tinted_surface(color: u32, dark: bool) -> u32 {
    if dark {
        super::colors::mix(color, 0xffffff, GLASS_TINT_DARK)
    } else {
        color
    }
}

/// 浮层亮边色（`0xRRGGBBAA`）：深色主题用 32% 白，浅色用 18%
/// 黑——在各自底色上都是一道读得出的细边，又不至于像描边。
pub fn glass_edge_rgba(dark: bool) -> u32 {
    if dark { 0xffff_ff52 } else { 0x0000_002e }
}

/// 玻璃填充色：`0xRRGGBBAA`，直接喂 `gpui::rgba`。
/// 关掉玻璃时 alpha 恒为 0xFF，即实色，与不透明底完全一致。
pub fn fill_rgba(enabled: bool, opacity: f32, color: u32) -> u32 {
    let alpha = if enabled {
        (clamp_opacity(opacity) * 255.).round() as u32
    } else {
        0xff
    };
    (color << 8) | alpha
}

/// 浮层玻璃填充色：先按主题磨砂 tint，再按透度给 alpha；
/// 关掉玻璃时 tint 也不做，回到实色底。
pub fn surface_rgba(enabled: bool, opacity: f32, color: u32, dark: bool) -> u32 {
    if enabled {
        fill_rgba(true, opacity, tinted_surface(color, dark))
    } else {
        fill_rgba(false, opacity, color)
    }
}

/// 终端底玻璃填充色：透度按终端下限 0.92 抬高，其余与浮层一致。
pub fn terminal_fill_rgba(enabled: bool, opacity: f32, color: u32) -> u32 {
    if enabled {
        fill_rgba(true, terminal_opacity(opacity), color)
    } else {
        fill_rgba(false, opacity, color)
    }
}

/// `surface` 以 `opacity` 盖在 `backdrop` 上之后的有效底色。
/// 只做对比度验算，不用于渲染；按 sRGB 通道线性混合，与渲染
/// 侧 `mix` 的行为一致。
#[cfg(test)]
pub(crate) fn composite_over(surface: u32, opacity: f32, backdrop: u32) -> u32 {
    mix(backdrop, surface, opacity)
}

/// `foreground` 文字落在玻璃面上时的实际对比度。
/// `backdrop` 是玻璃后面那层已经确定的颜色（终端底、画布或桌面）。
#[cfg(test)]
pub(crate) fn text_contrast(foreground: u32, surface: u32, opacity: f32, backdrop: u32) -> f64 {
    contrast(foreground, composite_over(surface, opacity, backdrop))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{MINIMUM_CONTRAST, Palette, ThemeId};

    #[test]
    fn opacity_clamps_into_range_and_recovers_from_nan() {
        assert_eq!(clamp_opacity(2.), GLASS_OPACITY_MAX);
        assert_eq!(clamp_opacity(0.), GLASS_OPACITY_MIN);
        assert_eq!(clamp_opacity(0.9), 0.9);
        assert_eq!(clamp_opacity(f32::NAN), GLASS_OPACITY_DEFAULT);
        assert_eq!(clamp_opacity(f32::INFINITY), GLASS_OPACITY_DEFAULT);
    }

    #[test]
    fn opacity_cycles_through_the_three_steps() {
        assert_eq!(next_opacity(0.85), 0.90);
        assert_eq!(next_opacity(0.90), 0.95);
        assert_eq!(next_opacity(0.95), 0.85);
        // 档外值就近向上进档，不跳回开头。
        assert_eq!(next_opacity(0.87), 0.90);
        assert_eq!(next_opacity(0.80), 0.85);
    }

    #[test]
    fn nearest_step_picks_the_closest_choice() {
        assert_eq!(nearest_step(0.85), 0);
        assert_eq!(nearest_step(0.90), 1);
        assert_eq!(nearest_step(0.95), 2);
        assert_eq!(nearest_step(0.88), 1);
    }

    #[test]
    fn terminal_opacity_floors_at_0_92() {
        assert_eq!(terminal_opacity(0.85), GLASS_TERMINAL_OPACITY_MIN);
        assert_eq!(terminal_opacity(0.90), GLASS_TERMINAL_OPACITY_MIN);
        assert_eq!(terminal_opacity(0.95), 0.95);
        assert_eq!(terminal_opacity(2.), 0.95);
    }

    #[test]
    fn tint_lifts_dark_surfaces_and_leaves_light_ones_alone() {
        assert_eq!(tinted_surface(0x1e222b, true), 0x353840);
        assert_eq!(tinted_surface(0xffffff, false), 0xffffff);
        assert_eq!(surface_rgba(true, 0.85, 0x1e222b, true), 0x35_38_40_d9);
        assert_eq!(surface_rgba(false, 0.85, 0x1e222b, true), 0x1e_22_2b_ff);
        assert_eq!(terminal_fill_rgba(true, 0.85, 0x14171d), 0x14_17_1d_eb);
        assert_eq!(terminal_fill_rgba(false, 0.85, 0x14171d), 0x14_17_1d_ff);
    }

    /// 0.85 的终端底保不住次要文字：Paper/Moss 的 `muted` 在纯
    /// 黑桌面上只有 4.08:1——这就是终端下限抬到 0.92 的原因，
    /// 不是拍脑袋，改动这个断言前先重算。
    #[test]
    fn terminal_opacity_0_85_fails_muted_on_a_black_desktop() {
        let paper = Palette::of(ThemeId::Paper);
        assert!(text_contrast(paper.muted, paper.terminal, 0.85, 0x000000) < MINIMUM_CONTRAST);
        assert!(text_contrast(paper.muted, paper.terminal, 0.90, 0x000000) >= MINIMUM_CONTRAST);
    }

    /// 终端文字对比度：三档透度换算成终端实际透度后 × 六套预设，
    /// 主文字与次要文字盖在纯黑/纯白桌面上都要达标。
    #[test]
    fn terminal_text_stays_readable_over_any_desktop() {
        for theme in ThemeId::PRESETS {
            let palette = Palette::of(theme);
            for (opacity, _) in GLASS_OPACITY_STEPS {
                let effective = terminal_opacity(opacity);
                for desktop in [0x000000, 0xffffff] {
                    for foreground in [palette.ink, palette.muted] {
                        let ratio = text_contrast(foreground, palette.terminal, effective, desktop);
                        assert!(
                            ratio >= MINIMUM_CONTRAST,
                            "{} #{foreground:06x} 终端文字（浮层档 {opacity}→终端 {effective}）：{ratio:.2}",
                            palette.id.name()
                        );
                    }
                }
            }
        }
    }

    /// 叠加最坏情况：浮层 0.85 + 整屏遮罩 + 终端底 0.92 + 纯黑
    /// 桌面。Paper 的报错文字 4.58:1，余量薄但仍在 4.5 之上。
    #[test]
    fn stacked_glass_layers_stay_readable() {
        let scrim_color = GLASS_SCRIM_RGBA >> 8;
        let scrim_alpha = (GLASS_SCRIM_RGBA & 0xff) as f32 / 255.;
        for theme in ThemeId::PRESETS {
            let palette = Palette::of(theme);
            let surface = tinted_surface(palette.window, palette.dark);
            for desktop in [0x000000, 0xffffff] {
                let terminal = composite_over(palette.terminal, terminal_opacity(0.85), desktop);
                let scrimmed = composite_over(scrim_color, scrim_alpha, terminal);
                for backdrop in [terminal, scrimmed] {
                    for foreground in [palette.ink, palette.muted, palette.accent, palette.error] {
                        let ratio = text_contrast(foreground, surface, 0.85, backdrop);
                        assert!(
                            ratio >= MINIMUM_CONTRAST,
                            "{} #{foreground:06x} 叠加玻璃（黑白桌面）：{ratio:.2}",
                            palette.id.name()
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn fill_alpha_follows_opacity_and_is_opaque_when_disabled() {
        let color = 0x1e222b;
        assert_eq!(fill_rgba(true, 0.85, color), 0x1e_22_2b_d9);
        assert_eq!(fill_rgba(true, 0.95, color), 0x1e_22_2b_f2);
        assert_eq!(fill_rgba(false, 0.85, color), 0x1e_22_2b_ff);
        // 关掉玻璃时透度值不影响结果。
        assert_eq!(fill_rgba(false, 0.95, color), 0x1e_22_2b_ff);
    }

    #[test]
    fn composite_over_is_the_identity_at_both_ends() {
        assert_eq!(composite_over(0x1e222b, 1., 0xffffff), 0x1e222b);
        assert_eq!(composite_over(0x1e222b, 0., 0xffffff), 0xffffff);
    }

    /// 玻璃上的文字对比度：三档透度 × 六套预设逐套断言。
    /// 场景按玻璃后面那层分三档，越往后越严：
    /// 浮层（命令面板、搜索条、弹窗）：盖在终端底上，命令面板与
    /// 弹窗还多一层整屏遮罩；
    /// 侧栏（Agent 侧栏、设置导航栏）：盖在画布上，画布本身以同
    /// 样透度透出桌面，黑白两端桌面都要算；
    /// 主文字另加「任意底色」兜底：单层玻璃直接盖纯黑/纯白桌面。
    #[test]
    fn glass_text_stays_readable_in_every_preset() {
        for theme in ThemeId::PRESETS {
            let palette = Palette::of(theme);
            for (opacity, _) in GLASS_OPACITY_STEPS {
                assert_glass_readable(&palette, opacity);
            }
        }
    }

    fn assert_glass_readable(palette: &Palette, opacity: f32) {
        let name = palette.id.name();
        // 浮层文字（ink / muted / accent / error）：终端底与遮罩压暗后的终端底。
        // 遮罩色与不透明度从生产用的同一个常量拆出来，避免测试与实现各写一份。
        let scrim_color = GLASS_SCRIM_RGBA >> 8;
        let scrim_alpha = (GLASS_SCRIM_RGBA & 0xff) as f32 / 255.;
        let scrimmed = composite_over(scrim_color, scrim_alpha, palette.terminal);
        // 浮层按 tint 后的底色验算（深色提亮会吃掉一点对比度）。
        let surface = tinted_surface(palette.window, palette.dark);
        for backdrop in [palette.terminal, scrimmed] {
            for foreground in [palette.ink, palette.muted, palette.accent, palette.error] {
                let ratio = text_contrast(foreground, surface, opacity, backdrop);
                assert!(
                    ratio >= MINIMUM_CONTRAST,
                    "{name} #{foreground:06x} 玻璃浮层（透度 {opacity}）：{ratio:.2}"
                );
            }
        }
        // 侧栏文字（ink / muted）：画布 + 桌面两层叠加，桌面取黑白两端。
        // 侧栏底色同样按 tint 后的值验算。
        let sidebar = tinted_surface(palette.sidebar, palette.dark);
        for desktop in [0x000000, 0xffffff] {
            let canvas = composite_over(palette.canvas, opacity, desktop);
            for foreground in [palette.ink, palette.muted] {
                let ratio = text_contrast(foreground, sidebar, opacity, canvas);
                assert!(
                    ratio >= MINIMUM_CONTRAST,
                    "{name} #{foreground:06x} 玻璃侧栏（透度 {opacity}）：{ratio:.2}"
                );
            }
        }
        // 主文字（ink）兜底：单层玻璃直接盖纯黑/纯白桌面。
        for desktop in [0x000000, 0xffffff] {
            let ratio = text_contrast(palette.ink, palette.window, opacity, desktop);
            assert!(
                ratio >= MINIMUM_CONTRAST,
                "{name} 主文字玻璃（透度 {opacity}）：{ratio:.2}"
            );
        }
    }
}
