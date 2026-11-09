// @Time    : 2026/11/5 15:12
// @Author  : fzf
// @FileName: session.rs
// @Software: Siskin

//! 会话的生命周期与创建参数。状态机显式建模，避免用 `Option` + `bool`
//! 拼出「已退出但仍能输入」这类非法组合。

use std::path::PathBuf;

use super::shell_spec::{ShellKind, ShellSpec};

/// 会话从创建到结束的全部状态。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SessionState {
    /// PTY 已请求派生，还没收到第一份输出。
    Starting,
    Running,
    Exited(ExitInfo),
    Failed(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ExitInfo {
    pub code: Option<i32>,
}

impl ExitInfo {
    pub(crate) fn from_status(code: Option<i32>) -> Self {
        Self { code }
    }

    pub(crate) fn label(self) -> String {
        match self.code {
            Some(code) => format!("已退出 · {code}"),
            None => "已退出".to_string(),
        }
    }
}

impl SessionState {
    pub(crate) fn is_running(&self) -> bool {
        matches!(self, SessionState::Starting | SessionState::Running)
    }

    /// 侧栏与状态栏共用的一行文案。
    pub(crate) fn label(&self) -> String {
        match self {
            SessionState::Starting => "启动中".to_string(),
            SessionState::Running => "运行中".to_string(),
            SessionState::Exited(info) => info.label(),
            SessionState::Failed(message) => format!("启动失败：{message}"),
        }
    }
}

/// 新建会话需要的全部信息。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SessionSpec {
    pub title: String,
    pub shell: ShellSpec,
    pub working_directory: Option<PathBuf>,
}

impl SessionSpec {
    pub(crate) fn new(title: impl Into<String>, kind: ShellKind) -> Self {
        Self {
            title: title.into(),
            shell: kind.spec(),
            working_directory: None,
        }
    }

    /// `-e <program> [args...]` 的会话：退出即关闭窗口。
    pub(crate) fn command(title: impl Into<String>, program: PathBuf, args: Vec<String>) -> Self {
        Self {
            title: title.into(),
            shell: ShellSpec::WithArgs { program, args },
            working_directory: None,
        }
    }

    pub(crate) fn with_directory(mut self, directory: Option<PathBuf>) -> Self {
        self.working_directory = directory;
        self
    }
}
