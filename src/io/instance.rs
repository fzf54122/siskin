// @Time    : 2026/11/5 21:45
// @Author  : fzf
// @FileName: instance.rs
// @Software: Siskin

//! 单实例。第二个进程只把参数转发给已经运行的实例然后退出，
//! 不创建第二个 GPU 上下文——那样会多占一份显存和 PTY。
//!
//! 用 D-Bus 名字而不是锁文件：名字由会话总线仲裁，进程崩溃时自动释放，
//! 不会留下需要用户手动删除的陈旧锁。

use zbus::blocking::{Connection, connection};
use zbus::interface;

pub(crate) const BUS_NAME: &str = "io.github.fzf.Siskin.Instance";
const OBJECT_PATH: &str = "/io/github/fzf/Siskin/Instance";
const INTERFACE: &str = "io.github.fzf.Siskin.Instance";

/// 第二个进程的结果。转发成功就不再启动窗口。
pub(crate) enum Secondary {
    /// 已有实例并已转发。
    Forwarded,
    /// 没有已有实例，可以继续启动。
    Owner(InstanceServer),
}

/// 持有总线连接。字段本身不被读取，存在的意义是保持名字注册，
/// drop 时由 zbus 自动释放。
pub(crate) struct InstanceServer {
    _connection: Connection,
}

/// 主实例暴露的最小接口：接收工作目录与「新窗口」请求。
struct InstanceService;

#[interface(name = "io.github.fzf.Siskin.Instance")]
impl InstanceService {
    /// 第二个进程把参数交过来。返回 `true` 表示已接受。
    fn activate(&self, working_directory: Option<String>) -> bool {
        // 目前只有一个窗口，落到统一的自定义事件里由主线程处理。
        let _ = working_directory;
        if let Ok(mut queue) = NEW_WINDOW_REQUESTS.lock() {
            queue.push(true);
        }
        true
    }
}

/// 待处理请求。用原子标志而不是通道：主线程每帧轮询一次即可，
/// 不需要为跨线程通信再引一个依赖。
static NEW_WINDOW_REQUESTS: std::sync::Mutex<Vec<bool>> = std::sync::Mutex::new(Vec::new());

/// 主线程轮询：有新窗口请求时返回 `true` 并清空。
pub(crate) fn take_new_window_request() -> bool {
    match NEW_WINDOW_REQUESTS.lock() {
        Ok(mut queue) => !queue.drain(..).next().is_none(),
        Err(_) => false,
    }
}

/// 尝试成为主实例。名字已被占用时转发参数并返回 `Forwarded`。
pub(crate) fn claim(working_directory: Option<String>) -> Secondary {
    match connection::Builder::session() {
        Ok(builder) => match builder
            .name(BUS_NAME)
            .and_then(|builder| builder.serve_at(OBJECT_PATH, InstanceService))
            .and_then(|builder| builder.build())
        {
            Ok(connection) => Secondary::Owner(InstanceServer {
                _connection: connection,
            }),
            // 名字被占用：转发给已有实例。连接失败也当作已有实例，
            // 避免因为 D-Bus 不可用而启动第二个进程。
            Err(_) => {
                forward(working_directory);
                Secondary::Forwarded
            }
        },
        Err(_) => Secondary::Forwarded,
    }
}

fn forward(working_directory: Option<String>) {
    let Ok(connection) = connection::Builder::session().and_then(|builder| builder.build()) else {
        return;
    };
    let Ok(proxy) = zbus::blocking::Proxy::new(&connection, BUS_NAME, OBJECT_PATH, INTERFACE)
    else {
        return;
    };
    let _ = proxy.call_method("Activate", &(working_directory,));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_requests_are_drained_once() {
        // 只测队列语义，不连 D-Bus：测试环境未必有会话总线。
        if let Ok(mut queue) = NEW_WINDOW_REQUESTS.lock() {
            queue.clear();
            queue.push(true);
        }
        assert!(take_new_window_request());
        assert!(!take_new_window_request());
    }
}
