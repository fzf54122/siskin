// @Time    : 2026/11/9 11:20
// @Author  : fzf
// @FileName: inputs.rs
// @Software: Siskin

//! 网格渲染的入参打包。参数有十几个，逐个位置传容易错位，
//! 打包后 `TerminalGrid` 只在 `new` 时读一次。

use gpui::{Entity, FocusHandle, SharedString};

use crate::domain::RenderMetrics;
use crate::io::search::MatchSpan;
use crate::io::vt::Frame;

use super::{CursorSlot, OriginSlot, TerminalGrid};

/// 渲染一帧需要的全部输入。
pub(crate) struct GridInputs {
    pub frame: Frame,
    /// 渲染时的亚行平移（像素）。不足一行的滚轮余量靠它补连续。
    pub offset_y: f32,
    pub metrics: RenderMetrics,
    pub font: SharedString,
    pub palette_bg: u32,
    pub palette_fg: u32,
    pub selection_bg: u32,
    pub highlight_bg: u32,
    pub match_bg: u32,
    pub cursor: u32,
    pub focused: bool,
    /// OSC 8 链接底线颜色，用主题强调色。
    pub link: u32,
    pub highlights: Vec<MatchSpan>,
    pub current_match: Option<MatchSpan>,
    pub cursor_slot: CursorSlot,
    pub origin_slot: OriginSlot,
    pub view: Entity<crate::app::SiskinApp>,
    pub focus: FocusHandle,
}

impl GridInputs {
    pub(crate) fn into_element(self) -> TerminalGrid {
        TerminalGrid::new(self)
    }
}
