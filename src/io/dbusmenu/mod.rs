// @Time    : 2026/11/5 10:40
// @Author  : fzf
// @FileName: mod.rs
// @Software: Siskin

//! Canonical dbusmenu export for desktop panels. KDE properties use STRING,
//! not UTF8_STRING. GTK menu properties are deliberately absent: org.gtk.Menus
//! is a different protocol and must not point at this object.

mod items;
mod layout;
mod service;

#[cfg(test)]
mod tests;

pub(crate) use items::MenuItem;
pub(crate) use layout::build_layout;

use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

use async_channel::{Receiver, Sender};
use zbus::blocking::{Proxy, connection::Builder};
use zbus::zvariant::ObjectPath;

use service::MenuService;

use crate::domain::Action;

pub(crate) const PATH: &str = "/org/siskin/Terminal/Menu";
pub(crate) const INTERFACE: &str = "com.canonical.dbusmenu";
const REGISTRAR: &str = "com.canonical.AppMenu.Registrar";
const REGISTRAR_PATH: &str = "/com/canonical/AppMenu/Registrar";

pub(crate) mod ids {
    pub const ROOT: i32 = 0;
    pub const FILE: i32 = 1;
    pub const NEW_SESSION: i32 = 2;
    pub const CLOSE_SESSION: i32 = 3;
    pub const CLEAR: i32 = 4;
    pub const QUIT: i32 = 5;
    pub const EDIT: i32 = 10;
    pub const COMMAND_PALETTE: i32 = 11;
    pub const SETTINGS: i32 = 12;
    pub const VIEW: i32 = 20;
    pub const THEME_MODE: i32 = 21;
    pub const FONT_LARGER: i32 = 22;
    pub const FONT_SMALLER: i32 = 23;
    pub const TOGGLE_INSPECTOR: i32 = 24;
    pub const SHELL: i32 = 30;
    pub const SHELL_ZSH: i32 = 31;
    pub const SHELL_BASH: i32 = 32;
    pub const SHELL_FISH: i32 = 33;
    pub const SHELL_SH: i32 = 34;
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum MenuState {
    #[default]
    Unavailable,
    Starting,
    Exported,
}

impl MenuState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Unavailable => "全局菜单不可用",
            Self::Starting => "正在导出全局菜单",
            Self::Exported => "全局菜单已导出",
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum MenuEvent {
    Action(Action),
    State(MenuState),
}

pub(crate) struct Model {
    pub root: MenuItem,
    pub revision: u32,
}

/// The UI handle owns the worker's command sender. Dropping it disconnects the
/// worker, unregisters the window on the same bus connection, and releases it.
pub(crate) struct GlobalMenu {
    model: Arc<Mutex<Model>>,
    updates: mpsc::SyncSender<()>,
    events: Receiver<MenuEvent>,
}

impl GlobalMenu {
    pub fn events(&self) -> Receiver<MenuEvent> {
        self.events.clone()
    }

    pub fn set_layout(&self, root: MenuItem) {
        let Ok(mut model) = self.model.lock() else {
            eprintln!("siskin: 全局菜单状态锁不可用");
            return;
        };
        if model.root == root {
            return;
        }
        model.root = root;
        model.revision = model.revision.wrapping_add(1);
        drop(model);
        // Layout changes coalesce; the worker always reads the latest model.
        match self.updates.try_send(()) {
            Ok(()) | Err(mpsc::TrySendError::Full(())) => {}
            Err(mpsc::TrySendError::Disconnected(())) => {
                eprintln!("siskin: 全局菜单消息线程已退出");
            }
        }
    }
}

/// Starts without blocking the GUI. zbus's async-io connection drives incoming
/// calls on its own executor; this worker owns its lifetime and outbound updates.
pub(crate) fn install(window_id: u32, root: MenuItem) -> std::io::Result<GlobalMenu> {
    let model = Arc::new(Mutex::new(Model { root, revision: 1 }));
    let (updates, commands) = mpsc::sync_channel(1);
    let (sender, events) = async_channel::bounded(64);
    let shared = model.clone();
    std::thread::Builder::new()
        .name("siskin-appmenu".into())
        .spawn(move || {
            if let Err(error) = serve(window_id, shared, sender.clone(), commands) {
                eprintln!("siskin: 无法导出全局菜单：{error}");
                let _ = sender.try_send(MenuEvent::State(MenuState::Unavailable));
            }
        })?;
    Ok(GlobalMenu {
        model,
        updates,
        events,
    })
}

fn serve(
    window_id: u32,
    model: Arc<Mutex<Model>>,
    events: Sender<MenuEvent>,
    commands: mpsc::Receiver<()>,
) -> Result<(), Box<dyn std::error::Error>> {
    let connection = Builder::session()?
        .method_timeout(Duration::from_secs(2))
        .build()?;
    let service_name = format!("org.siskin.Terminal.p{}", std::process::id());
    connection.request_name(service_name.as_str())?;
    connection.object_server().at(
        PATH,
        MenuService {
            model: model.clone(),
            events: events.clone(),
        },
    )?;
    let unique = connection
        .unique_name()
        .ok_or("Session bus did not assign a unique name")?
        .to_string();
    write_window_properties(window_id, &unique, PATH)?;
    events.try_send(MenuEvent::State(MenuState::Exported))?;

    let mut pump = Pump {
        connection: &connection,
        model: &model,
        commands: &commands,
        bus: &zbus::blocking::fdo::DBusProxy::new(&connection)?,
        registrar: &Proxy::new(&connection, REGISTRAR, REGISTRAR_PATH, REGISTRAR)?,
        window_id,
        path: ObjectPath::try_from(PATH)?,
        registered_owner: None,
    };
    while pump.step()? {}
    pump.unregister();
    Ok(())
}

/// 注册与更新共用的通道。参数按结构体打包，`serve` 每轮只调一个方法。
struct Pump<'a> {
    connection: &'a zbus::blocking::Connection,
    model: &'a Mutex<Model>,
    commands: &'a mpsc::Receiver<()>,
    bus: &'a zbus::blocking::fdo::DBusProxy<'a>,
    registrar: &'a Proxy<'a>,
    window_id: u32,
    path: ObjectPath<'a>,
    registered_owner: Option<String>,
}

impl Pump<'_> {
    /// 一轮「重注册 + 派发更新」，返回 `false` 表示命令通道已断开、该退出。
    fn step(&mut self) -> Result<bool, Box<dyn std::error::Error>> {
        // A new owner means the panel/registrar restarted. Re-register without
        // touching desktop services or losing our exported menu.
        let owner = self
            .bus
            .get_name_owner(REGISTRAR.try_into()?)
            .ok()
            .map(|name| name.to_string());
        if owner.is_some() && owner != self.registered_owner {
            match self
                .registrar
                .call_method("RegisterWindow", &(self.window_id, &self.path))
            {
                Ok(_) => self.registered_owner = owner,
                Err(error) => eprintln!("siskin: 注册桌面菜单失败，将重试：{error}"),
            }
        } else if owner.is_none() {
            self.registered_owner = None;
        }
        match self.commands.recv_timeout(Duration::from_secs(1)) {
            Ok(()) => {
                let revision = self
                    .model
                    .lock()
                    .map_err(|error| error.to_string())?
                    .revision;
                self.connection.emit_signal(
                    None::<&str>,
                    PATH,
                    INTERFACE,
                    "LayoutUpdated",
                    &(revision, ids::ROOT),
                )?;
                Ok(true)
            }
            Err(mpsc::RecvTimeoutError::Timeout) => Ok(true),
            Err(mpsc::RecvTimeoutError::Disconnected) => Ok(false),
        }
    }

    fn unregister(&self) {
        if self.registered_owner.is_none() {
            return;
        }
        if let Err(error) = self
            .registrar
            .call_method("UnregisterWindow", &(self.window_id,))
        {
            eprintln!("siskin: 注销桌面菜单失败：{error}");
        }
    }
}

fn write_window_properties(
    window_id: u32,
    unique_name: &str,
    object_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use x11rb::connection::Connection as _;
    use x11rb::protocol::xproto::{AtomEnum, ConnectionExt as _, PropMode};
    use x11rb::wrapper::ConnectionExt as _;

    let (connection, _) = x11rb::connect(None)?;
    for (name, value) in [
        ("_KDE_NET_WM_APPMENU_SERVICE_NAME", unique_name),
        ("_KDE_NET_WM_APPMENU_OBJECT_PATH", object_path),
    ] {
        let atom = connection
            .intern_atom(false, name.as_bytes())?
            .reply()?
            .atom;
        connection
            .change_property8(
                PropMode::REPLACE,
                window_id,
                atom,
                AtomEnum::STRING,
                value.as_bytes(),
            )?
            .check()?;
    }
    connection.flush()?;
    Ok(())
}
