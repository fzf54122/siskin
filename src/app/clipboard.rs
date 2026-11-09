// @Time    : 2026/11/5 18:30
// @Author  : fzf
// @FileName: clipboard.rs
// @Software: Siskin

//! 剪贴板桥接。`io` 层不认识 GPUI，复制与粘贴都在应用层落到系统剪贴板；
//! OSC 52 只允许程序「写」，读取系统剪贴板的能力不交给终端应用。

use gpui::{Context, Keystroke};

use super::SiskinApp;

impl SiskinApp {
    /// 把会话攒下的复制请求写进系统剪贴板。每帧调用一次，通常是空操作。
    pub(crate) fn sync_clipboard(&mut self, cx: &mut Context<Self>) {
        for text in self.take_clipboard_writes() {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
        }
        for text in self.take_primary_writes() {
            #[cfg(any(target_os = "linux", target_os = "freebsd"))]
            cx.write_to_primary(gpui::ClipboardItem::new_string(text));
            #[cfg(not(any(target_os = "linux", target_os = "freebsd")))]
            {
                let _ = text;
            }
        }
    }

    /// 终端自己的剪贴板快捷键。未命中时不消费，交给后续按键链路。
    pub(crate) fn sync_clipboard_shortcut(
        &mut self,
        keystroke: &Keystroke,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.editor_clipboard(keystroke, cx) {
            return true;
        }
        if self.page != crate::settings::Page::Terminal
            || self.palette.is_some()
            || self.search.is_some()
            || self.close_request.is_some()
        {
            return false;
        }
        let key = keystroke.key.as_str();
        let ctrl = keystroke.modifiers.control || keystroke.modifiers.platform;
        if ctrl && keystroke.modifiers.shift && key == "c" {
            if let Some(text) = self.copy_selection_text() {
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
            }
            return true;
        }
        if (ctrl && keystroke.modifiers.shift && key == "v")
            || (keystroke.modifiers.shift && key == "insert")
        {
            if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                self.paste_text(&text);
            }
            return true;
        }
        false
    }

    /// 中键粘贴使用 PRIMARY。Wayland 下可能没有该选区，缺失时静默跳过。
    pub(crate) fn paste_primary(&mut self, cx: &mut Context<Self>) {
        #[cfg(any(target_os = "linux", target_os = "freebsd"))]
        if let Some(text) = cx.read_from_primary().and_then(|item| item.text()) {
            self.paste_text(&text);
        }
        #[cfg(not(any(target_os = "linux", target_os = "freebsd")))]
        {
            let _ = cx;
        }
    }
}
