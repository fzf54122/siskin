// @Time    : 2026/11/11
// @Author  : fzf
// @FileName: pty_channel.rs
// @Software: Siskin

//! 读线程的消息通道。`mpsc` 送信后往唤醒管道写一个字节，
//! 让阻塞在 `poll` 里的读线程立刻醒来处理。

use std::borrow::Cow;
use std::os::unix::io::RawFd;
use std::sync::mpsc::{Receiver, Sender};

/// 发给读线程的消息。`Input` 写回 PTY，`Shutdown` 退出线程。
#[derive(Debug)]
pub(crate) enum PtyMsg {
    Input(Cow<'static, [u8]>),
    Shutdown,
}

/// 读线程的发送端。发送失败（线程已退出）返回 `false`，不 panic。
pub(crate) struct PtyChannel {
    sender: Sender<PtyMsg>,
    wake_write: RawFd,
}

impl PtyChannel {
    /// 送信并唤醒 `poll`。接收端建好后才能发，见 `new_channel`。
    pub(crate) fn send(&self, msg: PtyMsg) -> bool {
        if self.sender.send(msg).is_err() {
            return false;
        }
        let byte = [1u8];
        // SAFETY: `wake_write` 是合法的管道写端，单字节写不阻塞。
        let _ = unsafe { libc::write(self.wake_write, byte.as_ptr() as *const libc::c_void, 1) };
        true
    }
}

impl Drop for PtyChannel {
    fn drop(&mut self) {
        // SAFETY: fd 只在这里关闭一次，`Pty` 不克隆发送端。
        unsafe { libc::close(self.wake_write) };
    }
}

/// 建一对通道：发送端给 `Pty`，接收端与唤醒管道读端给读线程。
pub(crate) fn new_channel() -> std::io::Result<(PtyChannel, Receiver<PtyMsg>, RawFd)> {
    let (sender, receiver) = std::sync::mpsc::channel();
    let (wake_read, wake_write) = make_wake_pipe()?;
    Ok((PtyChannel { sender, wake_write }, receiver, wake_read))
}

fn make_wake_pipe() -> std::io::Result<(RawFd, RawFd)> {
    let mut fds = [0; 2];
    // SAFETY: `pipe` 成功时填满两个合法 fd，失败返回错误。
    if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    for fd in fds {
        set_nonblock(fd);
    }
    Ok((fds[0], fds[1]))
}

fn set_nonblock(fd: RawFd) {
    // SAFETY: `fd` 是刚创建的合法管道端，只改自己的标志。
    unsafe {
        let flags = libc::fcntl(fd, libc::F_GETFL, 0);
        libc::fcntl(
            fd,
            libc::F_SETFL,
            flags | libc::O_NONBLOCK | libc::O_CLOEXEC,
        );
    }
}
