// @Time    : 2026/11/5 21:12
// @Author  : fzf
// @FileName: session_state.rs
// @Software: Siskin

//! 会话恢复用的最小快照：只记标题与工作目录，不记输出。
//! 记输出既占体积，也会让「恢复」变成伪造历史。

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SessionSnapshot {
    pub title: String,
    #[serde(default)]
    pub working_directory: Option<String>,
}

impl SessionSnapshot {
    pub(crate) fn new(
        title: impl Into<String>,
        working_directory: Option<std::path::PathBuf>,
    ) -> Self {
        Self {
            title: title.into(),
            working_directory: working_directory.and_then(|path| path.to_str().map(str::to_string)),
        }
    }

    pub(crate) fn directory(&self) -> Option<std::path::PathBuf> {
        self.working_directory
            .as_ref()
            .map(std::path::PathBuf::from)
            .filter(|path| path.is_dir())
    }

    /// 目录不存在（换机器、删了项目）时不要恢复，否则 shell 会启动在错误位置。
    pub(crate) fn is_restorable(&self) -> bool {
        self.working_directory.is_none() || self.directory().is_some()
    }
}
