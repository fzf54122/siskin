// @Time    : 2026/11/5 10:40
// @Author  : fzf
// @FileName: dbusmenu.rs
// @Software: Siskin

//! Canonical dbusmenu export for desktop panels. KDE properties use STRING,
//! not UTF8_STRING. GTK menu properties are deliberately absent: org.gtk.Menus
//! is a different protocol and must not point at this object.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

use async_channel::{Receiver, Sender};
use zbus::blocking::{Proxy, connection::Builder};
use zbus::interface;
use zbus::zvariant::{OwnedValue, Str, Value};

use crate::settings::ShellKind;

const PATH: &str = "/org/siskin/Terminal/Menu";
const INTERFACE: &str = "com.canonical.dbusmenu";
const REGISTRAR: &str = "com.canonical.AppMenu.Registrar";
const REGISTRAR_PATH: &str = "/com/canonical/AppMenu/Registrar";

pub mod ids {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuAction {
    NewSession,
    CloseSession,
    Clear,
    Quit,
    CommandPalette,
    Settings,
    CycleThemeMode,
    FontLarger,
    FontSmaller,
    ToggleInspector,
    SetShell(ShellKind),
}

impl MenuAction {
    pub fn from_id(id: i32) -> Option<Self> {
        use ids::*;
        Some(match id {
            NEW_SESSION => Self::NewSession,
            CLOSE_SESSION => Self::CloseSession,
            CLEAR => Self::Clear,
            QUIT => Self::Quit,
            COMMAND_PALETTE => Self::CommandPalette,
            SETTINGS => Self::Settings,
            THEME_MODE => Self::CycleThemeMode,
            FONT_LARGER => Self::FontLarger,
            FONT_SMALLER => Self::FontSmaller,
            TOGGLE_INSPECTOR => Self::ToggleInspector,
            SHELL_ZSH => Self::SetShell(ShellKind::Zsh),
            SHELL_BASH => Self::SetShell(ShellKind::Bash),
            SHELL_FISH => Self::SetShell(ShellKind::Fish),
            SHELL_SH => Self::SetShell(ShellKind::Sh),
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MenuState {
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
pub enum MenuEvent {
    Action(MenuAction),
    State(MenuState),
}

type Properties = HashMap<String, OwnedValue>;
type LayoutNode = (i32, Properties, Vec<OwnedValue>);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuItem {
    id: i32,
    label: String,
    children: Vec<MenuItem>,
    separator: bool,
    toggle: Option<(&'static str, bool)>,
}

impl MenuItem {
    fn item(id: i32, label: impl Into<String>) -> Self {
        Self {
            id,
            label: label.into(),
            children: Vec::new(),
            separator: false,
            toggle: None,
        }
    }

    fn submenu(id: i32, label: &str, children: Vec<Self>) -> Self {
        Self {
            children,
            ..Self::item(id, label)
        }
    }

    fn separator(id: i32) -> Self {
        Self {
            separator: true,
            ..Self::item(id, "")
        }
    }

    fn checked(id: i32, label: &str, kind: &'static str, checked: bool) -> Self {
        Self {
            toggle: Some((kind, checked)),
            ..Self::item(id, label)
        }
    }

    fn find(&self, id: i32) -> Option<&Self> {
        if self.id == id {
            Some(self)
        } else {
            self.children.iter().find_map(|child| child.find(id))
        }
    }

    fn properties(&self, names: &[String]) -> Properties {
        let mut props = Properties::new();
        props.insert("label".into(), text_value(&self.label));
        props.insert("enabled".into(), true.into());
        props.insert("visible".into(), true.into());
        if self.separator {
            props.insert("type".into(), text_value("separator"));
        }
        if !self.children.is_empty() {
            props.insert("children-display".into(), text_value("submenu"));
        }
        if let Some((kind, checked)) = self.toggle {
            props.insert("toggle-type".into(), text_value(kind));
            props.insert("toggle-state".into(), i32::from(checked).into());
        }
        if !names.is_empty() {
            props.retain(|key, _| names.contains(key));
        }
        props
    }

    fn layout(&self, depth: i32, names: &[String]) -> zbus::fdo::Result<LayoutNode> {
        let children = if depth == 0 {
            Vec::new()
        } else {
            let next = if depth < 0 { -1 } else { depth - 1 };
            self.children
                .iter()
                .map(|child| {
                    let layout = child.layout(next, names)?;
                    OwnedValue::try_from(Value::from(layout))
                        .map_err(|error| zbus::fdo::Error::Failed(error.to_string()))
                })
                .collect::<Result<Vec<_>, _>>()?
        };
        Ok((self.id, self.properties(names), children))
    }
}

struct Model {
    root: MenuItem,
    revision: u32,
}

/// The UI handle owns the worker's command sender. Dropping it disconnects the
/// worker, unregisters the window on the same bus connection, and releases it.
pub struct GlobalMenu {
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

struct MenuService {
    model: Arc<Mutex<Model>>,
    events: Sender<MenuEvent>,
}

impl MenuService {
    fn dispatch(&self, id: i32, event_id: &str) -> zbus::fdo::Result<()> {
        if event_id != "clicked" {
            return Ok(());
        }
        let Some(action) = MenuAction::from_id(id) else {
            return Err(zbus::fdo::Error::InvalidArgs(format!(
                "Unknown menu item {id}"
            )));
        };
        self.events
            .try_send(MenuEvent::Action(action))
            .map_err(|error| zbus::fdo::Error::Failed(error.to_string()))
    }
}

#[interface(name = "com.canonical.dbusmenu")]
impl MenuService {
    #[zbus(property)]
    fn version(&self) -> u32 {
        4
    }

    #[zbus(property)]
    fn text_direction(&self) -> &str {
        "ltr"
    }

    #[zbus(property)]
    fn status(&self) -> &str {
        "normal"
    }

    #[zbus(property)]
    fn icon_theme_path(&self) -> Vec<String> {
        Vec::new()
    }

    fn get_layout(
        &self,
        parent_id: i32,
        recursion_depth: i32,
        property_names: Vec<String>,
    ) -> zbus::fdo::Result<(u32, LayoutNode)> {
        let model = self
            .model
            .lock()
            .map_err(|error| zbus::fdo::Error::Failed(error.to_string()))?;
        let item = model.root.find(parent_id).ok_or_else(|| {
            zbus::fdo::Error::InvalidArgs(format!("Unknown menu item {parent_id}"))
        })?;
        Ok((
            model.revision,
            item.layout(recursion_depth, &property_names)?,
        ))
    }

    fn get_group_properties(
        &self,
        ids: Vec<i32>,
        property_names: Vec<String>,
    ) -> zbus::fdo::Result<Vec<(i32, Properties)>> {
        let model = self
            .model
            .lock()
            .map_err(|error| zbus::fdo::Error::Failed(error.to_string()))?;
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

    fn get_property(&self, id: i32, name: &str) -> zbus::fdo::Result<OwnedValue> {
        let model = self
            .model
            .lock()
            .map_err(|error| zbus::fdo::Error::Failed(error.to_string()))?;
        model
            .root
            .find(id)
            .and_then(|item| item.properties(&[]).remove(name))
            .ok_or_else(|| zbus::fdo::Error::InvalidArgs(format!("Unknown property {id}:{name}")))
    }

    fn about_to_show(&self, _id: i32) -> bool {
        false
    }

    fn about_to_show_group(&self, _ids: Vec<i32>) -> (Vec<i32>, Vec<i32>) {
        (Vec::new(), Vec::new())
    }

    fn event(
        &self,
        id: i32,
        event_id: &str,
        _data: Value<'_>,
        _timestamp: u32,
    ) -> zbus::fdo::Result<()> {
        self.dispatch(id, event_id)
    }

    fn event_group(&self, events: Vec<(i32, String, OwnedValue, u32)>) -> Vec<i32> {
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

/// Starts without blocking the GUI. zbus's async-io connection drives incoming
/// calls on its own executor; this worker owns its lifetime and outbound updates.
pub fn install(window_id: u32, root: MenuItem) -> std::io::Result<GlobalMenu> {
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

    let registrar = Proxy::new(&connection, REGISTRAR, REGISTRAR_PATH, REGISTRAR)?;
    let bus = zbus::blocking::fdo::DBusProxy::new(&connection)?;
    let path = zbus::zvariant::ObjectPath::try_from(PATH)?;
    let mut registered_owner = None;
    loop {
        // A new owner means the panel/registrar restarted. Re-register without
        // touching desktop services or losing our exported menu.
        let owner = bus
            .get_name_owner(REGISTRAR.try_into()?)
            .ok()
            .map(|name| name.to_string());
        if owner.is_some() && owner != registered_owner {
            match registrar.call_method("RegisterWindow", &(window_id, &path)) {
                Ok(_) => registered_owner = owner,
                Err(error) => eprintln!("siskin: 注册桌面菜单失败，将重试：{error}"),
            }
        } else if owner.is_none() {
            registered_owner = None;
        }
        match commands.recv_timeout(Duration::from_secs(1)) {
            Ok(()) => {
                let revision = model.lock().map_err(|error| error.to_string())?.revision;
                connection.emit_signal(
                    None::<&str>,
                    PATH,
                    INTERFACE,
                    "LayoutUpdated",
                    &(revision, ids::ROOT),
                )?;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    if registered_owner.is_some()
        && let Err(error) = registrar.call_method("UnregisterWindow", &(window_id,))
    {
        eprintln!("siskin: 注销桌面菜单失败：{error}");
    }
    Ok(())
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

pub fn build_layout(theme: &str, shell: &str) -> MenuItem {
    use ids::*;
    MenuItem::submenu(
        ROOT,
        "",
        vec![
            MenuItem::submenu(
                FILE,
                "文件",
                vec![
                    MenuItem::item(NEW_SESSION, "新建会话"),
                    MenuItem::item(CLOSE_SESSION, "关闭当前会话"),
                    MenuItem::separator(101),
                    MenuItem::item(CLEAR, "清空滚动区"),
                    MenuItem::separator(102),
                    MenuItem::item(QUIT, "退出 Siskin"),
                ],
            ),
            MenuItem::submenu(
                EDIT,
                "编辑",
                vec![
                    MenuItem::item(COMMAND_PALETTE, "命令面板"),
                    MenuItem::item(SETTINGS, "设置…"),
                ],
            ),
            MenuItem::submenu(
                VIEW,
                "视图",
                vec![
                    MenuItem::item(THEME_MODE, format!("主题：{theme}（切换模式）")),
                    MenuItem::item(FONT_LARGER, "放大字号"),
                    MenuItem::item(FONT_SMALLER, "缩小字号"),
                    MenuItem::separator(120),
                    MenuItem::item(TOGGLE_INSPECTOR, "显示/隐藏会话信息"),
                    MenuItem::submenu(
                        SHELL,
                        "Shell（演示配置）",
                        vec![
                            MenuItem::checked(SHELL_ZSH, "zsh", "radio", shell == "zsh"),
                            MenuItem::checked(SHELL_BASH, "bash", "radio", shell == "bash"),
                            MenuItem::checked(SHELL_FISH, "fish", "radio", shell == "fish"),
                            MenuItem::checked(SHELL_SH, "sh", "radio", shell == "sh"),
                        ],
                    ),
                ],
            ),
        ],
    )
}

fn text_value(text: &str) -> OwnedValue {
    OwnedValue::from(Str::from(text.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use zbus::zvariant::Structure;

    fn service() -> (MenuService, Receiver<MenuEvent>) {
        let (events, receiver) = async_channel::bounded(64);
        (
            MenuService {
                model: Arc::new(Mutex::new(Model {
                    root: build_layout("跟随系统", "zsh"),
                    revision: 1,
                })),
                events,
            },
            receiver,
        )
    }

    fn child(value: OwnedValue) -> LayoutNode {
        Structure::try_from(value).unwrap().try_into().unwrap()
    }

    #[test]
    fn submenus_advertise_children_and_respect_parent_depth_and_filter() {
        let (menu, _) = service();
        let (_, root) = menu.get_layout(0, 1, vec![]).unwrap();
        assert_eq!(root.2.len(), 3);
        let file = child(root.2.into_iter().next().unwrap());
        assert_eq!(file.0, ids::FILE);
        assert_eq!(
            <&str>::try_from(&file.1["children-display"]).unwrap(),
            "submenu"
        );
        assert!(file.2.is_empty());
        let (_, file) = menu.get_layout(ids::FILE, 1, vec!["label".into()]).unwrap();
        assert_eq!(file.0, ids::FILE);
        assert_eq!(file.2.len(), 6);
        assert_eq!(file.1.len(), 1);
        assert!(menu.get_layout(9999, -1, vec![]).is_err());
    }

    #[test]
    fn every_click_action_has_a_real_menu_item_and_toggle_type() {
        let (menu, events) = service();
        for id in [ids::NEW_SESSION, ids::SETTINGS, ids::SHELL_FISH] {
            menu.event(id, "clicked", 0i32.into(), 0).unwrap();
            assert_eq!(
                events.try_recv().unwrap(),
                MenuEvent::Action(MenuAction::from_id(id).unwrap())
            );
        }
        menu.event(ids::SETTINGS, "opened", 0i32.into(), 0).unwrap();
        assert!(events.try_recv().is_err());
        assert!(menu.event(999, "clicked", 0i32.into(), 0).is_err());
        assert_eq!(
            <&str>::try_from(
                menu.get_property(ids::SHELL_ZSH, "toggle-type")
                    .as_ref()
                    .unwrap()
            )
            .unwrap(),
            "radio"
        );
    }

    #[test]
    fn protocol_round_trip_on_an_isolated_bus() {
        use std::io::{BufRead, BufReader};
        use std::process::{Command, Stdio};

        // GPUI tests install process-global runtime hooks. Keep the real
        // socket-reactor test in its own process, not on that mock runtime.
        if std::env::var_os("SISKIN_PROTOCOL_TEST_CHILD").is_none() {
            let output = Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "dbusmenu::tests::protocol_round_trip_on_an_isolated_bus",
                    "--nocapture",
                ])
                .env("SISKIN_PROTOCOL_TEST_CHILD", "1")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }

        struct TestBus(std::process::Child);
        impl Drop for TestBus {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
        // Never register test services with the user's desktop session.
        let mut bus = TestBus(
            Command::new("dbus-daemon")
                .args(["--session", "--nofork", "--print-address=1"])
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        );
        let mut address = String::new();
        BufReader::new(bus.0.stdout.take().unwrap())
            .read_line(&mut address)
            .unwrap();
        let address = address.trim();
        let server = Builder::address(address)
            .unwrap()
            .method_timeout(Duration::from_secs(2))
            .build()
            .unwrap();
        let (menu, events) = service();
        server.object_server().at(PATH, menu).unwrap();
        let client = Builder::address(address)
            .unwrap()
            .method_timeout(Duration::from_secs(2))
            .build()
            .unwrap();
        let proxy = Proxy::new(
            &client,
            server.unique_name().unwrap().as_str(),
            PATH,
            INTERFACE,
        )
        .unwrap();

        let (revision, file): (u32, LayoutNode) = proxy
            .call("GetLayout", &(ids::FILE, 1i32, Vec::<String>::new()))
            .unwrap();
        assert_eq!(revision, 1);
        assert_eq!(file.0, ids::FILE);
        assert_eq!(file.2.len(), 6);
        assert_eq!(proxy.get_property::<u32>("Version").unwrap(), 4);
        let _: () = proxy
            .call(
                "Event",
                &(ids::SETTINGS, "clicked", Value::from(0i32), 0u32),
            )
            .unwrap();
        assert_eq!(
            events.try_recv().unwrap(),
            MenuEvent::Action(MenuAction::Settings)
        );
        let about: bool = proxy.call("AboutToShow", &(ids::FILE,)).unwrap();
        assert!(!about);
    }
}
