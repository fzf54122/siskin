// @Time    : 2026/11/5 13:22
// @Author  : fzf
// @FileName: tests.rs
// @Software: Siskin

//! 全局菜单协议。`MenuService` 的接口方法由 zbus 宏生成且只在本模块内
//! 可见，所以测试贴着模块放；最后一个用例起隔离的 session bus 做真实往返。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use zbus::blocking::{Proxy, connection::Builder};
use zbus::zvariant::{OwnedValue, Structure, Value};

use super::service::{MenuService, action_from_id};
use super::{INTERFACE, MenuEvent, MenuState, Model, PATH, build_layout, ids};
use crate::domain::{Action, ShellKind};

type Node = (i32, HashMap<String, OwnedValue>, Vec<OwnedValue>);

fn service() -> (MenuService, async_channel::Receiver<MenuEvent>) {
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

fn child(value: OwnedValue) -> Node {
    let structure = Structure::try_from(value).expect("布局子项是结构体");
    structure.try_into().expect("布局子项形如 (id, 属性, 子项)")
}

#[test]
fn submenus_advertise_children_and_respect_parent_depth_and_filter() {
    let (menu, _) = service();
    let (_, root) = menu.get_layout(ids::ROOT, 1, vec![]).unwrap();
    assert_eq!(root.2.len(), 3);
    let file = child(root.2.into_iter().next().unwrap());
    assert_eq!(file.0, ids::FILE);
    assert_eq!(
        <&str>::try_from(&file.1["children-display"]).unwrap(),
        "submenu"
    );
    // 递归深度为 0 时只回当前层，子菜单里不能再带子项。
    assert!(file.2.is_empty());
    let (_, file) = menu.get_layout(ids::FILE, 1, vec!["label".into()]).unwrap();
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
            MenuEvent::Action(action_from_id(id).unwrap())
        );
    }
    // 非 clicked 事件不该产生动作。
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

/// 菜单 id 与动作必须双向对齐：每个可点击项都有动作，未知 id 没有动作。
#[test]
fn menu_ids_and_actions_stay_in_sync_with_the_layout() {
    let root = build_layout("深色", "bash");
    for id in [
        ids::NEW_SESSION,
        ids::CLOSE_SESSION,
        ids::CLEAR,
        ids::QUIT,
        ids::COMMAND_PALETTE,
        ids::SETTINGS,
        ids::THEME_MODE,
        ids::FONT_LARGER,
        ids::FONT_SMALLER,
        ids::TOGGLE_INSPECTOR,
        ids::SHELL_ZSH,
        ids::SHELL_BASH,
        ids::SHELL_FISH,
        ids::SHELL_SH,
    ] {
        assert!(root.find(id).is_some(), "菜单缺少 id {id}");
        assert!(action_from_id(id).is_some(), "id {id} 没有对应动作");
    }
    assert_eq!(
        action_from_id(ids::SHELL_BASH),
        Some(Action::SetShell(ShellKind::Bash))
    );
    assert_eq!(action_from_id(ids::FILE), None);
    assert_eq!(MenuState::default(), MenuState::Unavailable);
}

/// 隔离的 session bus。`Drop` 里 kill，测试失败也不会留下守护进程。
struct TestBus(std::process::Child);

impl Drop for TestBus {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// GPUI 测试会安装进程级 runtime hook，真实 socket 的往返必须放到子进程，
/// 否则会落在 mock runtime 上。父进程只负责派发并校验子进程退出码。
fn rerun_in_child() {
    use std::process::Command;

    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "io::dbusmenu::tests::protocol_round_trip_on_an_isolated_bus",
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
}

/// 绝不把测试服务注册到用户桌面会话：每次起一个 --session 守护进程，
/// 只监听它自己打印的私有地址。
fn isolated_bus() -> (TestBus, String) {
    use std::io::{BufRead, BufReader};
    use std::process::{Command, Stdio};

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
    let address = address.trim().to_string();
    assert!(
        !address.is_empty(),
        "dbus-daemon 没有打印监听地址，可能缺少可用的 socket"
    );
    // 地址行出现在 socket 已经 listen 之后，但守护进程还要初始化完
    // 认证与策略；立刻连接在高并发下会连接到半就绪的 daemon。
    std::thread::sleep(std::time::Duration::from_millis(50));
    (bus, address)
}

/// 建连接并做一次自检。
///
/// zbus 的内部 executor 线程在 `build()` 之后才启动，退出条件是
/// `is_empty()`；高负载下它会抢在 socket 读取任务注册之前看到空队列并
/// 退出，连接此后不再派发任何消息（实测整包并行跑约 1/5 概率）。自检用
/// 一次总线调用暴露这种情况，失败就重建连接，而不是让断言随机超时。
fn connect(address: &str) -> zbus::blocking::Connection {
    for _ in 0..3 {
        let connection = Builder::address(address)
            .unwrap()
            .method_timeout(Duration::from_secs(2))
            .build()
            .unwrap();
        let alive = connection.call_method(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            Some("org.freedesktop.DBus"),
            "GetId",
            &(),
        );
        if alive.is_ok() {
            return connection;
        }
    }
    panic!("三次连接都没有可用的消息派发线程");
}

#[test]
fn protocol_round_trip_on_an_isolated_bus() {
    if std::env::var_os("SISKIN_PROTOCOL_TEST_CHILD").is_none() {
        rerun_in_child();
        return;
    }

    let (_bus, address) = isolated_bus();
    let server = connect(&address);
    let (menu, events) = service();
    server.object_server().at(PATH, menu).unwrap();
    let client = connect(&address);
    let proxy = Proxy::new(
        &client,
        server.unique_name().unwrap().as_str(),
        PATH,
        INTERFACE,
    )
    .unwrap();

    let (revision, file): (u32, Node) = proxy
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
        MenuEvent::Action(Action::OpenSettings)
    );
    let about: bool = proxy.call("AboutToShow", &(ids::FILE,)).unwrap();
    assert!(!about);
}
