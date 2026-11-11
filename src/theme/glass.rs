// @Time    : 2026/11/11
// @Author  : fzf
// @FileName: glass.rs
// @Software: Siskin

//! 磨砂玻璃：浮层与侧栏的半透明底色。
//!
//! 核实结论（全文见 `docs/glass.md`）：
//! - GPUI 没有逐元素的 backdrop blur，只有窗口级
//!   `WindowBackgroundAppearance::Blurred`；
//! - Wayland 下它走 KDE 的 `org_kde_kwin_blur` 协议，只在 KWin
//!   实现，GNOME/Mutter 上静默退化为普通透明；
//! - X11 下 `Blurred` 与 `Transparent` 等价，只开透明不请求模糊；
//! - pulse-ui v0.1.1 没有任何 blur / backdrop / glass 接口。
//!
//! 所以玻璃是「半透明填充 + 细边框 + 分层透度」的近似，只在 KDE
//! Plasma Wayland 上能叠加一道真模糊；终端主区始终实底，不参与。
//!
//! 透度下限不是拍脑袋的：对六套预设逐套验算，0.75 时 Paper 的浮层
//! 文字只有 4.34:1、Moss 的侧栏次要文字只有 4.30:1，都跌破 WCAG
//! 4.5:1；0.80 时 Paper 只剩 4.50 零余量；0.85 起最差组合 4.67:1
//! 才有余量。因此下限就是 0.85，三档是 0.85 / 0.90 / 0.95。

#[cfg(test)]
use super::colors::{contrast, mix};

/// 浮层底色不透明度下限。低于它，浅色主题在深色桌面/遮罩下的文字
/// 对比度会跌破 4.5:1（见模块头注释的验算）。
pub const GLASS_OPACITY_MIN: f32 = 0.85;

/// 浮层底色不透明度上限。再浓就和实色没有区别。
pub const GLASS_OPACITY_MAX: f32 = 0.95;

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
        for backdrop in [palette.terminal, scrimmed] {
            for foreground in [palette.ink, palette.muted, palette.accent, palette.error] {
                let ratio = text_contrast(foreground, palette.window, opacity, backdrop);
                assert!(
                    ratio >= MINIMUM_CONTRAST,
                    "{name} #{foreground:06x} 玻璃浮层（透度 {opacity}）：{ratio:.2}"
                );
            }
        }
        // 侧栏文字（ink / muted）：画布 + 桌面两层叠加，桌面取黑白两端。
        for desktop in [0x000000, 0xffffff] {
            let canvas = composite_over(palette.canvas, opacity, desktop);
            for foreground in [palette.ink, palette.muted] {
                let ratio = text_contrast(foreground, palette.sidebar, opacity, canvas);
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
