// @Time    : 2026/11/4 23:30
// @Author  : fzf
// @FileName: fonts.rs
// @Software: Siskin

pub const MONO_FONTS: [&str; 6] = [
    "JetBrains Mono",
    "MesloLGL Nerd Font Mono",
    "Fira Code",
    "DejaVu Sans Mono",
    "Noto Sans Mono",
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
        if let Some(first) = installed.iter().find(|name| name.contains("Mono")) {
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
