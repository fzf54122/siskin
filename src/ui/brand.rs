// @Time    : 2026/11/5 18:20
// @Author  : fzf
// @FileName: brand.rs
// @Software: Siskin

//! 侧栏与窗口共用同一份 PNG。窗口图标在 `main.rs` 里直接解码，
//! 这里只负责缩放到侧栏尺寸，避免两套品牌图形走样。
//!
//! 侧栏图标使用完整方形资源，避免 app icon 内部留白把鸟形缩得太小。

use std::sync::{Arc, OnceLock};

use gpui::{ImageSource, Img, RenderImage, img};

use super::*;

pub(super) fn brand_mark(size: f32) -> Img {
    static MARK: OnceLock<Option<Arc<RenderImage>>> = OnceLock::new();
    let source = MARK.get_or_init(|| {
        match image::load_from_memory(include_bytes!("../../assets/icons/siskin-64.png")) {
            Ok(image) => Some(Arc::new(RenderImage::new(vec![image::Frame::new(
                image.to_rgba8(),
            )]))),
            Err(error) => {
                eprintln!("siskin: 无法解码侧栏图标：{error}");
                None
            }
        }
    });
    let element = match source {
        Some(image) => img(ImageSource::Render(Arc::clone(image))),
        None => img(ImageSource::Render(Arc::new(RenderImage::new(vec![
            image::Frame::new(image::RgbaImage::new(1, 1)),
        ])))),
    };
    element.size(px(size)).flex_shrink_0()
}
