// @Time    : 2026/11/5 19:40
// @Author  : fzf
// @FileName: shape.rs
// @Software: Siskin

//! 单行/单格文本整形。字体与颜色解析集中在这里，网格定位只关心坐标。

use std::sync::Arc;

use gpui::{Font, FontStyle, FontWeight, SharedString, TextRun, UnderlineStyle, Window, px};

use crate::io::vt::CellStyle;

use super::to_hsla;

pub(super) fn text_style(family: &SharedString, style: CellStyle) -> (Font, gpui::Hsla) {
    let font = Font {
        family: family.clone(),
        weight: if style.bold {
            FontWeight::BOLD
        } else {
            FontWeight::default()
        },
        style: if style.italic {
            FontStyle::Italic
        } else {
            FontStyle::default()
        },
        ..Font::default()
    };
    (font, to_hsla(style.fg))
}

/// 一段文本按自身推进量整形，保留连字；不强制等宽，
/// 否则 CJK 与组合字符会互相重叠。
pub(super) fn shape_text(
    family: &SharedString,
    text: &str,
    style: CellStyle,
    font_size: f32,
    window: &mut Window,
) -> Arc<gpui::ShapedLine> {
    let text_system = window.text_system().clone();
    let (font, color) = text_style(family, style);
    let shaped = text_system.shape_line(
        SharedString::from(text.to_string()),
        px(font_size),
        &[TextRun {
            len: text.len(),
            font,
            color,
            background_color: None,
            underline: style.underline.then(|| UnderlineStyle {
                color: Some(color),
                thickness: px(1.),
                ..Default::default()
            }),
            strikethrough: None,
        }],
        None,
    );
    Arc::new(shaped)
}
