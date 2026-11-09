// @Time    : 2026/11/5 15:20
// @Author  : fzf
// @FileName: ansi.rs
// @Software: Siskin

//! ANSI 颜色解析。`alacritty_terminal` 只告诉我们「这是第 3 号命名色」
//! 或「这是 color cube 里的第 129 号」，具体 RGB 由主题决定；这里负责把
//! 索引换算成调色板里的颜色。

use alacritty_terminal::vte::ansi::NamedColor;
#[cfg(test)]
use alacritty_terminal::vte::ansi::{Color, Rgb};

use crate::theme::Palette;

/// 主题里的 16 色表。默认值按主题明暗分别推导，用户主题只影响前后景。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AnsiColors {
    pub normal: [u32; 8],
    pub bright: [u32; 8],
    pub foreground: u32,
    pub background: u32,
    pub cursor: u32,
}

impl AnsiColors {
    pub(crate) fn from_palette(palette: &Palette) -> Self {
        let normal = if palette.dark {
            [
                0x1b1e25, 0xe06c75, 0x98c379, 0xe5c07b, 0x61afef, 0xc678dd, 0x56b6c2, 0xabb2bf,
            ]
        } else {
            [
                0x3b3d42, 0x9e5962, 0x557459, 0x8f703f, 0x496c9b, 0x87618d, 0x42777b, 0x70757e,
            ]
        };
        let bright = if palette.dark {
            [
                0x5c6370, 0xff7b86, 0xb5e890, 0xf0d197, 0x82c4ff, 0xdc9bf5, 0x7ee0e8, 0xffffff,
            ]
        } else {
            [
                0x62666f, 0x934951, 0x46694c, 0x7b6036, 0x3c5c88, 0x785580, 0x35696f, 0x9399a2,
            ]
        };
        Self {
            normal,
            bright,
            foreground: palette.ink,
            background: palette.terminal,
            cursor: palette.accent,
        }
    }

    /// 命名色 → RGB。`Foreground`/`Background`/`Cursor` 用主题色，
    /// `Dim*` 落到对应的普通色，避免出现「暗色不可见」。
    pub(crate) fn named(&self, color: NamedColor) -> u32 {
        match color {
            NamedColor::Black => self.normal[0],
            NamedColor::Red => self.normal[1],
            NamedColor::Green => self.normal[2],
            NamedColor::Yellow => self.normal[3],
            NamedColor::Blue => self.normal[4],
            NamedColor::Magenta => self.normal[5],
            NamedColor::Cyan => self.normal[6],
            NamedColor::White => self.normal[7],
            NamedColor::BrightBlack => self.bright[0],
            NamedColor::BrightRed => self.bright[1],
            NamedColor::BrightGreen => self.bright[2],
            NamedColor::BrightYellow => self.bright[3],
            NamedColor::BrightBlue => self.bright[4],
            NamedColor::BrightMagenta => self.bright[5],
            NamedColor::BrightCyan => self.bright[6],
            NamedColor::BrightWhite => self.bright[7],
            NamedColor::Foreground | NamedColor::BrightForeground => self.foreground,
            NamedColor::Background => self.background,
            NamedColor::Cursor => self.cursor,
            NamedColor::DimBlack => self.normal[0],
            NamedColor::DimRed => self.normal[1],
            NamedColor::DimGreen => self.normal[2],
            NamedColor::DimYellow => self.normal[3],
            NamedColor::DimBlue => self.normal[4],
            NamedColor::DimMagenta => self.normal[5],
            NamedColor::DimCyan => self.normal[6],
            NamedColor::DimWhite => self.normal[7],
            NamedColor::DimForeground => self.foreground,
        }
    }

    pub(crate) fn indexed(&self, index: u8) -> u32 {
        match index {
            0..=7 => self.normal[index as usize],
            8..=15 => self.bright[index as usize - 8],
            // 16..=231 是 6×6×6 色立方，232..=255 是 24 级灰阶。
            16..=231 => {
                let index = index as u32 - 16;
                let steps = [0u8, 95, 135, 175, 215, 255];
                let red = steps[(index / 36) as usize];
                let green = steps[((index / 6) % 6) as usize];
                let blue = steps[(index % 6) as usize];
                rgb_hex(red, green, blue)
            }
            _ => {
                let level = 8 + (index as u32 - 232) * 10;
                let level = level.min(255) as u8;
                rgb_hex(level, level, level)
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn resolve(&self, color: Color) -> u32 {
        match color {
            Color::Named(named) => self.named(named),
            Color::Spec(Rgb { r, g, b }) => rgb_hex(r, g, b),
            Color::Indexed(index) => self.indexed(index),
        }
    }
}

fn rgb_hex(red: u8, green: u8, blue: u8) -> u32 {
    ((red as u32) << 16) | ((green as u32) << 8) | blue as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::PAPER;

    fn colors() -> AnsiColors {
        AnsiColors::from_palette(&PAPER)
    }

    #[test]
    fn named_colors_map_to_sixteen_distinct_values() {
        let table = colors();
        let mut seen = Vec::new();
        for index in 0..16u8 {
            let color = if index < 8 {
                Color::Named(named_index(index))
            } else {
                Color::Indexed(index)
            };
            let value = table.resolve(color);
            assert!(!seen.contains(&value), "16 色表出现重复：{value:06x}");
            seen.push(value);
        }
    }

    fn named_index(index: u8) -> NamedColor {
        const ORDER: [NamedColor; 8] = [
            NamedColor::Black,
            NamedColor::Red,
            NamedColor::Green,
            NamedColor::Yellow,
            NamedColor::Blue,
            NamedColor::Magenta,
            NamedColor::Cyan,
            NamedColor::White,
        ];
        ORDER[index as usize]
    }

    #[test]
    fn color_cube_and_grayscale_follow_the_xterm_ramp() {
        let table = colors();
        assert_eq!(table.indexed(16), 0x000000);
        assert_eq!(table.indexed(231), 0xffffff);
        assert_eq!(table.indexed(232), 0x080808);
        assert_eq!(table.indexed(255), 0xeeeeee);
        assert_eq!(table.indexed(196), 0xff0000);
    }

    #[test]
    fn truecolor_passes_through_untouched() {
        let table = colors();
        let color = Color::Spec(Rgb {
            r: 0x12,
            g: 0x34,
            b: 0x56,
        });
        assert_eq!(table.resolve(color), 0x123456);
    }

    #[test]
    fn foreground_and_background_follow_the_palette() {
        let table = colors();
        assert_eq!(
            table.resolve(Color::Named(NamedColor::Foreground)),
            PAPER.ink
        );
        assert_eq!(
            table.resolve(Color::Named(NamedColor::Background)),
            PAPER.terminal
        );
    }
}
