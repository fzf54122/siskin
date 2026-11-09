// @Time    : 2026/11/5 12:32
// @Author  : fzf
// @FileName: menu_ops.rs
// @Software: Siskin

//! 全局菜单的生命周期与事件桥接。菜单树构造在 `io::dbusmenu`，这里只负责
//! 「应用状态 → 菜单」和「菜单事件 → 动作」两个方向的转换。

use gpui::Context;

use crate::domain::ActionEffect;
use crate::io::dbusmenu::{self, MenuEvent, MenuState};

use super::SiskinApp;

impl SiskinApp {
    /// Rebuilds the menu exported to the desktop panel. Called at startup and
    /// whenever a menu-visible setting changes.
    pub(crate) fn sync_menu(&mut self) {
        let Some(menu) = self.menu.as_ref() else {
            return;
        };
        menu.set_layout(self.menu_layout());
    }

    fn menu_layout(&self) -> dbusmenu::MenuItem {
        dbusmenu::build_layout(
            self.settings.mode.name(),
            self.sessions.active().kind.name(),
        )
    }

    pub(crate) fn install_menu(&mut self, window_id: u32, cx: &mut Context<Self>) {
        match dbusmenu::install(window_id, self.menu_layout()) {
            Ok(menu) => {
                let events = menu.events();
                self.menu = Some(menu);
                self.menu_state = MenuState::Starting;
                self.attach_menu_events(events, cx);
            }
            Err(error) => eprintln!("siskin: 无法启动全局菜单：{error}"),
        }
    }

    pub(crate) fn attach_menu_events(
        &mut self,
        events: async_channel::Receiver<MenuEvent>,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |this, cx| {
            while let Ok(event) = events.recv().await {
                if this
                    .update(cx, |this, cx| this.apply_menu_event(event, cx))
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
    }

    pub(crate) fn apply_menu_event(&mut self, event: MenuEvent, cx: &mut Context<Self>) {
        match event {
            MenuEvent::Action(action) => {
                if action.run(self) == ActionEffect::Quit {
                    cx.quit();
                }
            }
            MenuEvent::State(state) => self.menu_state = state,
        }
        cx.notify();
    }
}
