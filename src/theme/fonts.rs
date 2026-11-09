// @Time    : 2026/11/4 23:30
// @Author  : fzf
// @FileName: fonts.rs
// @Software: Siskin

pub const MONO_FONTS: [&str; 6] = [
    // Nerd Font 排在前面：Powerlevel10k 之类的提示符依赖它的图标字形，
    // 换成不含图标的字体时提示符会显示成方块。
    "MesloLGL Nerd Font Mono",
    "JetBrains Mono",
    "Noto Sans Mono",
    "Fira Code",
    "DejaVu Sans Mono",
    "Liberation Mono",
];

pub const UI_FONTS: [&str; 5] = [
    "Inter",
    "Noto Sans CJK SC",
    "Source Han Sans SC",
    "Noto Sans",
    "DejaVu Sans",
];

/// Installed monospace families in preference order, always non-empty.
pub fn mono_fonts(installed: &[String]) -> Vec<String> {
    let mut fonts: Vec<String> = MONO_FONTS
        .iter()
        .filter(|candidate| installed.iter().any(|name| name == *candidate))
        .map(|name| name.to_string())
        .collect();
    if fonts.is_empty() {
        if let Some(first) = installed.iter().find(|name| name.ends_with("Mono")) {
            fonts.push(first.clone());
        } else {
            fonts.push("monospace".to_string());
        }
    }
    fonts
}

/// Chrome text uses a UI face; only terminal cells stay monospaced.
pub fn ui_family(installed: &[String]) -> &'static str {
    UI_FONTS
        .iter()
        .copied()
        .find(|candidate| installed.iter().any(|name| name == candidate))
        .unwrap_or(".SystemUIFont")
}
