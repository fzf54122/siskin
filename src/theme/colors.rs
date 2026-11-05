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
