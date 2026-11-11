// @Time    : 2026/11/9 11:20
// @Author  : fzf
// @FileName: inputs.rs
// @Software: Siskin

//! 网格渲染的入参打包。参数有十几个，逐个位置传容易错位，
//! 打包后 `TerminalGrid` 只在 `new` 时读一次。

use std::sync::Arc;

use gpui::{Entity, FocusHandle, SharedString};

use crate::domain::RenderMetrics;
use crate::io::search::MatchSpan;
use crate::io::vt::Frame;

use super::{CursorSlot, OriginSlot, TerminalGrid};

/// 渲染一帧需要的全部输入。
pub(crate) struct GridInputs {
    /// 共享快照：与会话、搜索同一份 `Arc`，构造时只克隆指针。
    pub frame: Arc<Frame>,
    /// 渲染时的亚行平移（像素）。不足一行的滚轮余量靠它补连续。
    pub offset_y: f32,
    pub metrics: RenderMetrics,
    pub font: SharedString,
    pub palette_bg: u32,
    /// 终端底色的不透明度：整窗玻璃开着时低于 1（下限 0.92），
    /// 关掉玻璃时为 1。底色矩形按这个画，整窗才真的透。
    pub bg_alpha: f32,
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
    /// 行内自动补全的后缀文本（光标之后、弱化色画的那段）。
    /// `None` 表示没有联想；位置由帧光标定，不在这里再传行列。
    pub ghost: Option<String>,
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
