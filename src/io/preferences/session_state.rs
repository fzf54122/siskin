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
    /// 用户显式改的标签名。老快照没有这个字段，`#[serde(default)]`
    /// 让它按 `None` 加载，显示回退到目录名，不丢数据。
    #[serde(default)]
    pub custom_title: Option<String>,
}

impl SessionSnapshot {
    pub(crate) fn new(
        title: impl Into<String>,
        working_directory: Option<std::path::PathBuf>,
        custom_title: Option<String>,
    ) -> Self {
        Self {
            title: title.into(),
            working_directory: working_directory.and_then(|path| path.to_str().map(str::to_string)),
            custom_title,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_keeps_the_explicit_name() {
        let snapshot = SessionSnapshot::new(
            "终端",
            Some(std::path::PathBuf::from("/tmp")),
            Some("工作".into()),
        );
        let json = serde_json::to_string(&snapshot).unwrap();
        let loaded: SessionSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded, snapshot);
        assert_eq!(loaded.custom_title.as_deref(), Some("工作"));
    }

    #[test]
    fn snapshots_without_the_new_field_load_with_no_explicit_name() {
        // 老快照只有 title 与 working_directory：缺字段按 None 加载，
        // 显示回退到目录名，不丢数据、不报错。
        let loaded: SessionSnapshot =
            serde_json::from_str(r#"{"title":"终端","working_directory":"/tmp"}"#).unwrap();
        assert_eq!(loaded.title, "终端");
        assert_eq!(loaded.custom_title, None);
        let minimal: SessionSnapshot = serde_json::from_str(r#"{"title":"终端"}"#).unwrap();
        assert_eq!(minimal.working_directory, None);
        assert_eq!(minimal.custom_title, None);
    }
}
