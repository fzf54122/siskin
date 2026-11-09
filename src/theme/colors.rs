// @Time    : 2026/11/4 23:30
// @Author  : fzf
// @FileName: colors.rs
// @Software: Siskin

/// Accent choices for the custom theme. The palette derives everything else
/// from the base tone plus this accent.
pub const ACCENTS: [(u32, &str); 6] = [
    (0x3c6e53, "苔绿"),
    (0x3f6ea8, "靛蓝"),
    (0x8a5a9e, "紫藤"),
    (0xa6473f, "砖红"),
    (0x9a6a1e, "琥珀"),
    (0x34817d, "青瓷"),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct CustomTheme {
    pub dark: bool,
    pub accent: usize,
    pub accent_rgb: Option<ThemeColor>,
    pub background: Option<ThemeColor>,
    pub foreground: Option<ThemeColor>,
}

impl Default for CustomTheme {
    fn default() -> Self {
        Self {
            dark: true,
            accent: 0,
            accent_rgb: None,
            background: None,
            foreground: None,
        }
    }
}

impl CustomTheme {
    pub fn accent_color(&self) -> u32 {
        self.accent_rgb.map_or_else(
            || ACCENTS[self.accent.min(ACCENTS.len() - 1)].0,
            |color| color.0,
        )
    }
}

/// 相对亮度（WCAG 2.0）。用于对比度计算，不用于渲染。
pub(crate) fn luminance(color: u32) -> f64 {
    let channel = |shift: u32| {
        let value = ((color >> shift) & 255u32) as f64 / 255.;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    channel(16) * 0.2126 + channel(8) * 0.7152 + channel(0) * 0.0722
}

/// WCAG 对比度，范围 1..21。1 是同色，21 是纯黑白。
pub(crate) fn contrast(a: u32, b: u32) -> f64 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

/// 把 `color` 往 `towards` 推，直到与 `background` 的对比度达到 `minimum`。
/// 返回调整后的颜色；已经达标时原样返回。
///
/// 迭代而不是一次到位：对比度对亮度不是线性响应，粗调容易过冲，
/// 把字色推到纯黑/纯白同样难看。
pub(crate) fn ensure_contrast(color: u32, background: u32, minimum: f64) -> u32 {
    if contrast(color, background) >= minimum {
        return color;
    }
    // 背景深就提亮前景，背景浅就压暗。方向选错会越调越糟。
    let towards = if luminance(background) < 0.5 {
        0xffffff
    } else {
        0x000000
    };
    let mut adjusted = color;
    // 每步 4% 足够细，60 步足以覆盖任何起止色对。
    for _ in 0..60 {
        adjusted = blend(adjusted, towards, 0.04);
        if contrast(adjusted, background) >= minimum {
            return adjusted;
        }
    }
    towards
}

/// 按比例混合两个颜色，`amount` 是取 `towards` 的比例。
pub(crate) fn blend(from: u32, towards: u32, amount: f64) -> u32 {
    let amount = amount.clamp(0., 1.);
    let channel = |shift: u32| {
        let a = ((from >> shift) & 255u32) as f64;
        let b = ((towards >> shift) & 255u32) as f64;
        ((a * (1. - amount) + b * amount).round() as u32).min(255)
    };
    (channel(16) << 16) | (channel(8) << 8) | channel(0)
}

/// Blends `a` toward `b`; `t` is how much of `b` to take.
pub(crate) fn mix(a: u32, b: u32, t: f32) -> u32 {
    let t = t.clamp(0., 1.);
    let channel = |shift: u32| {
        let left = ((a >> shift) & 0xff) as f32;
        let right = ((b >> shift) & 0xff) as f32;
        ((left * (1. - t) + right * t).round() as u32).min(0xff)
    };
    (channel(16) << 16) | (channel(8) << 8) | channel(0)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ThemeColor(pub u32);

impl TryFrom<String> for ThemeColor {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let hex = value.strip_prefix('#').unwrap_or(&value);
        if hex.len() != 6 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("颜色必须是 #RRGGBB 格式".into());
        }
        u32::from_str_radix(hex, 16)
            .map(Self)
            .map_err(|error| error.to_string())
    }
}

impl From<ThemeColor> for String {
    fn from(value: ThemeColor) -> Self {
        format!("#{:06x}", value.0)
    }
}
