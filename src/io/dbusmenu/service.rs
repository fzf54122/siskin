// @Time    : 2026/11/5 12:40
// @Author  : fzf
// @FileName: service.rs
// @Software: Siskin

//! `com.canonical.dbusmenu` 接口实现。这里只做「加锁读取模型」与
//! 「把点击转成动作」，不含菜单树构造。

use std::sync::{Arc, Mutex};

use async_channel::Sender;
use zbus::interface;
use zbus::zvariant::{OwnedValue, Value};

use super::items::{LayoutNode, Properties, ids};
use super::{MenuEvent, Model};
use crate::domain::{Action, ShellKind};

pub(crate) struct MenuService {
    pub model: Arc<Mutex<Model>>,
    pub events: Sender<MenuEvent>,
}

impl MenuService {
    fn dispatch(&self, id: i32, event_id: &str) -> zbus::fdo::Result<()> {
        if event_id != "clicked" {
            return Ok(());
        }
        let Some(action) = action_from_id(id) else {
            return Err(zbus::fdo::Error::InvalidArgs(format!(
                "Unknown menu item {id}"
            )));
        };
        self.events
            .try_send(MenuEvent::Action(action))
            .map_err(|error| zbus::fdo::Error::Failed(error.to_string()))
    }
}

/// 菜单 id 到动作的唯一映射。新增菜单项时这里必须同步，否则协议测试会失败。
pub(crate) fn action_from_id(id: i32) -> Option<Action> {
    use ids::*;
    Some(match id {
        NEW_SESSION => Action::NewSession,
        CLOSE_SESSION => Action::CloseSession,
        CLEAR => Action::Clear,
        QUIT => Action::Quit,
        COMMAND_PALETTE => Action::OpenPalette,
        SETTINGS => Action::OpenSettings,
        THEME_MODE => Action::CycleThemeMode,
        FONT_LARGER => Action::FontLarger,
        FONT_SMALLER => Action::FontSmaller,
        TOGGLE_INSPECTOR => Action::ToggleInspector,
        SHELL_ZSH => Action::SetShell(ShellKind::Zsh),
        SHELL_BASH => Action::SetShell(ShellKind::Bash),
        SHELL_FISH => Action::SetShell(ShellKind::Fish),
        SHELL_SH => Action::SetShell(ShellKind::Sh),
        _ => return None,
    })
}

#[interface(name = "com.canonical.dbusmenu")]
impl MenuService {
    #[zbus(property)]
    pub(crate) fn version(&self) -> u32 {
        4
    }

    #[zbus(property)]
    pub(crate) fn text_direction(&self) -> &str {
        "ltr"
    }

    #[zbus(property)]
    pub(crate) fn status(&self) -> &str {
        "normal"
    }

    #[zbus(property)]
    pub(crate) fn icon_theme_path(&self) -> Vec<String> {
        Vec::new()
    }

    pub(crate) fn get_layout(
        &self,
        parent_id: i32,
        recursion_depth: i32,
        property_names: Vec<String>,
    ) -> zbus::fdo::Result<(u32, LayoutNode)> {
        let model = self.lock()?;
        let item = model.root.find(parent_id).ok_or_else(|| {
            zbus::fdo::Error::InvalidArgs(format!("Unknown menu item {parent_id}"))
        })?;
        Ok((
            model.revision,
            item.layout(recursion_depth, &property_names)?,
        ))
    }

    pub(crate) fn get_group_properties(
        &self,
        ids: Vec<i32>,
        property_names: Vec<String>,
    ) -> zbus::fdo::Result<Vec<(i32, Properties)>> {
        let model = self.lock()?;
        Ok(ids
            .into_iter()
            .filter_map(|id| {
                model
                    .root
                    .find(id)
                    .map(|item| (id, item.properties(&property_names)))
            })
            .collect())
    }

    pub(crate) fn get_property(&self, id: i32, name: &str) -> zbus::fdo::Result<OwnedValue> {
        let model = self.lock()?;
        model
            .root
            .find(id)
            .and_then(|item| item.properties(&[]).remove(name))
            .ok_or_else(|| zbus::fdo::Error::InvalidArgs(format!("Unknown property {id}:{name}")))
    }

    // 候选菜单是静态的，不需要提前通知「即将展开」。
    pub(crate) fn about_to_show(&self, _id: i32) -> bool {
        false
    }

    pub(crate) fn about_to_show_group(&self, _ids: Vec<i32>) -> (Vec<i32>, Vec<i32>) {
        (Vec::new(), Vec::new())
    }

    pub(crate) fn event(
        &self,
        id: i32,
        event_id: &str,
        _data: Value<'_>,
        _timestamp: u32,
    ) -> zbus::fdo::Result<()> {
        self.dispatch(id, event_id)
    }

    pub(crate) fn event_group(&self, events: Vec<(i32, String, OwnedValue, u32)>) -> Vec<i32> {
        events
            .into_iter()
            .filter_map(|(id, event, _, _)| self.dispatch(id, &event).err().map(|_| id))
            .collect()
    }

    #[zbus(signal)]
    async fn layout_updated(
        emitter: &zbus::object_server::SignalEmitter<'_>,
        revision: u32,
        parent: i32,
    ) -> zbus::Result<()>;
}

impl MenuService {
    fn lock(&self) -> zbus::fdo::Result<std::sync::MutexGuard<'_, Model>> {
        self.model
            .lock()
            .map_err(|error| zbus::fdo::Error::Failed(error.to_string()))
    }
}
