// @Time    : 2026/11/5 17:42
// @Author  : fzf
// @FileName: icons.rs
// @Software: Siskin

use super::*;
use gpui::{PathBuilder, canvas, point};

#[derive(Clone, Copy)]
pub(super) enum ChromeIcon {
    Terminal,
    Plus,
    Close,
    Search,
    Back,
    Type,
    Palette,
    Desktop,
    Check,
    /// 三横线列表：Agent 侧栏的开关图标。
    List,
}

pub(super) fn chrome_icon(icon: ChromeIcon, color: u32) -> Div {
    let paths = icon_paths(icon);
    div().size(px(16.)).flex_shrink_0().child(
        canvas(
            |_, _, _| (),
            move |bounds, _, window, _| {
                let mut path = PathBuilder::stroke(px(1.4));
                for vertices in paths {
                    for (index, &(x, y)) in vertices.iter().enumerate() {
                        let vertex = bounds.origin
                            + point(
                                bounds.size.width * (x / 20.),
                                bounds.size.height * (y / 20.),
                            );
                        if index == 0 {
                            path.move_to(vertex);
                        } else {
                            path.line_to(vertex);
                        }
                    }
                }
                if let Ok(path) = path.build() {
                    window.paint_path(path, rgb(color));
                }
            },
        )
        .size_full(),
    )
}

fn icon_paths(icon: ChromeIcon) -> &'static [&'static [(f32, f32)]] {
    match icon {
        ChromeIcon::Terminal => &[&[(4., 6.), (8., 10.), (4., 14.)], &[(11., 14.), (16., 14.)]],
        ChromeIcon::Plus => &[&[(10., 4.), (10., 16.)], &[(4., 10.), (16., 10.)]],
        ChromeIcon::Close => &[&[(5., 5.), (15., 15.)], &[(15., 5.), (5., 15.)]],
        ChromeIcon::Search => &[
            &[(13., 13.), (17., 17.)],
            &[
                (8., 3.),
                (12., 4.),
                (14., 8.),
                (13., 12.),
                (9., 14.),
                (5., 12.),
                (3., 8.),
                (4., 5.),
                (8., 3.),
            ],
        ],
        ChromeIcon::Back => &[&[(16., 10.), (4., 10.), (9., 5.)], &[(4., 10.), (9., 15.)]],
        ChromeIcon::Type => &[
            &[(3., 16.), (8., 4.), (13., 16.)],
            &[(5., 12.), (11., 12.)],
            &[(14., 9.), (18., 9.)],
            &[(16., 9.), (16., 16.)],
        ],
        ChromeIcon::Palette => &[
            &[
                (17., 11.),
                (17., 6.),
                (13., 3.),
                (7., 3.),
                (3., 7.),
                (3., 13.),
                (7., 17.),
                (10., 17.),
                (11., 14.),
                (16., 14.),
                (17., 11.),
            ],
            &[(6., 7.), (6., 8.)],
            &[(10., 5.), (10., 6.)],
            &[(14., 8.), (14., 9.)],
        ],
        ChromeIcon::Desktop => &[
            &[(2., 3.), (18., 3.), (18., 14.), (2., 14.), (2., 3.)],
            &[(10., 14.), (10., 18.)],
            &[(7., 18.), (13., 18.)],
        ],
        ChromeIcon::Check => &[&[(4., 10.), (8., 14.), (16., 6.)]],
        ChromeIcon::List => &[
            &[(5., 6.), (15., 6.)],
            &[(5., 10.), (15., 10.)],
            &[(5., 14.), (15., 14.)],
        ],
    }
}
