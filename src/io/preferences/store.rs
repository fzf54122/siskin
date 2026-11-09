// @Time    : 2026/11/5 12:06
// @Author  : fzf
// @FileName: store.rs
// @Software: Siskin

//! 配置存储策略。文件实现与内存实现共用同一套解码/校验，测试不再落真实磁盘。

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use super::{MAX_BYTES, Preferences};

/// 存储失败。`Io` 保留原始错误，`Format` 只描述解码问题，两者都在外层转成
/// 中文提示后写入日志。
#[derive(Debug)]
pub(crate) enum StoreError {
    Io(std::io::Error),
    Format(String),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::Io(error) => write!(formatter, "{error}"),
            StoreError::Format(message) => write!(formatter, "{message}"),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<std::io::Error> for StoreError {
    fn from(error: std::io::Error) -> Self {
        StoreError::Io(error)
    }
}

impl From<serde_json::Error> for StoreError {
    fn from(error: serde_json::Error) -> Self {
        StoreError::Format(error.to_string())
    }
}

pub(crate) trait Store {
    fn load(&self) -> Result<Preferences, StoreError>;
    fn save(&self, preferences: &Preferences) -> Result<(), StoreError>;
    /// 用于界面提示，不参与读写决策。
    fn describe(&self) -> String;
}

/// `$XDG_CONFIG_HOME/siskin/settings.json`，未设置 XDG 时退回 `~/.config`。
#[derive(Clone, Debug)]
pub(crate) struct FileStore {
    path: PathBuf,
}

impl FileStore {
    pub(crate) fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    fn read_bytes(&self) -> Result<Option<Vec<u8>>, StoreError> {
        let file = match std::fs::File::open(&self.path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        // 多读一字节，用来区分「刚好上限」与「超限」。
        let mut bytes = Vec::new();
        file.take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err(StoreError::Format(format!(
                "设置文件超过 {} KiB",
                MAX_BYTES / 1024
            )));
        }
        Ok(Some(bytes))
    }
}

impl Store for FileStore {
    fn load(&self) -> Result<Preferences, StoreError> {
        match self.read_bytes()? {
            Some(bytes) => decode(&bytes, self.describe()),
            None => Ok(Preferences::default()),
        }
    }

    fn save(&self, preferences: &Preferences) -> Result<(), StoreError> {
        let parent = self
            .path
            .parent()
            .ok_or_else(|| StoreError::Format("设置路径没有父目录".into()))?;
        std::fs::create_dir_all(parent)?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        serde_json::to_writer_pretty(&mut temporary, preferences)?;
        temporary.write_all(b"\n")?;
        // 先落盘再替换，断电时最多丢掉这次设置，不会留下半个文件。
        temporary.as_file().sync_all()?;
        temporary.persist(&self.path).map_err(|error| error.error)?;
        Ok(())
    }

    fn describe(&self) -> String {
        self.path.display().to_string()
    }
}

/// 测试与「只读预览」用的内存实现。
#[cfg(test)]
#[derive(Clone, Debug, Default)]
pub(crate) struct MemoryStore {
    contents: std::sync::Arc<std::sync::Mutex<Option<String>>>,
}

#[cfg(test)]
impl MemoryStore {
    pub(crate) fn with_contents(contents: impl Into<String>) -> Self {
        Self {
            contents: std::sync::Arc::new(std::sync::Mutex::new(Some(contents.into()))),
        }
    }

    pub(crate) fn contents(&self) -> Option<String> {
        self.contents.lock().ok().and_then(|guard| guard.clone())
    }
}

#[cfg(test)]
impl Store for MemoryStore {
    fn load(&self) -> Result<Preferences, StoreError> {
        let guard = self
            .contents
            .lock()
            .map_err(|error| StoreError::Format(error.to_string()))?;
        match guard.as_deref() {
            Some(text) => decode(text.as_bytes(), "内存".into()),
            None => Ok(Preferences::default()),
        }
    }

    fn save(&self, preferences: &Preferences) -> Result<(), StoreError> {
        let mut text = serde_json::to_string_pretty(preferences)?;
        text.push('\n');
        let mut guard = self
            .contents
            .lock()
            .map_err(|error| StoreError::Format(error.to_string()))?;
        *guard = Some(text);
        Ok(())
    }

    fn describe(&self) -> String {
        "内存".into()
    }
}

/// 先按 `Value` 解析再迁移，最后反序列化。直接反序列化会要求文件
/// 已经是当前版本，旧文件就永远升不上来。
fn decode(bytes: &[u8], source: String) -> Result<Preferences, StoreError> {
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|error| StoreError::Format(format!("{source}: {error}")))?;
    // 最早的配置文件没有版本号，按 1 处理。
    let version = value
        .get("version")
        .and_then(serde_json::Value::as_u64)
        .map(|value| value as u32)
        .unwrap_or(1);
    let migrated = super::migrate(version, value)
        .and_then(super::migrate::stamp)
        .map_err(|error| StoreError::Format(format!("{source}: {error}")))?;
    let preferences: Preferences = serde_json::from_value(migrated)
        .map_err(|error| StoreError::Format(format!("{source}: {error}")))?;
    preferences
        .validate()
        .map_err(|message| StoreError::Format(format!("{source}: {message}")))
}
