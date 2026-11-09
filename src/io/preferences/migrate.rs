// @Time    : 2026/11/5 21:10
// @Author  : fzf
// @FileName: migrate.rs
// @Software: Siskin

//! 配置版本迁移。每一步只负责「上一版 → 这一版」的字段变化，
//! 跳跃版本（例如 1 直接到 3）直接报错，不允许静默用默认值填补。

use serde_json::{Map, Value};

use super::CURRENT_VERSION;

/// 迁移失败。`Unsupported` 表示文件来自更新版本，`Broken` 表示数据本身坏了。
#[derive(Debug)]
pub(crate) enum MigrateError {
    Unsupported(u32),
    Broken(String),
}

impl std::fmt::Display for MigrateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MigrateError::Unsupported(version) => write!(
                formatter,
                "配置版本 {version} 高于当前支持的 {CURRENT_VERSION}"
            ),
            MigrateError::Broken(message) => write!(formatter, "配置迁移失败：{message}"),
        }
    }
}

impl std::error::Error for MigrateError {}

/// 从任意旧版本升到当前版本。`version` 缺省时按 1 处理，
/// 因为最早的配置文件没有写版本号。
pub(crate) fn migrate(version: u32, value: Value) -> Result<Value, MigrateError> {
    if version > CURRENT_VERSION {
        return Err(MigrateError::Unsupported(version));
    }
    if version == 0 {
        return Err(MigrateError::Broken("版本号不能是 0".into()));
    }
    let mut value = value;
    let mut current = version;
    while current < CURRENT_VERSION {
        value = step(current, value)?;
        current += 1;
    }
    Ok(value)
}

/// 责任链的一环：`step(1, ..)` 把 v1 升成 v2。
fn step(from: u32, value: Value) -> Result<Value, MigrateError> {
    match from {
        1 => v1_to_v2(value),
        other => Err(MigrateError::Broken(format!("缺少 v{other} 的迁移步骤"))),
    }
}

/// v1 → v2：`shell` 从固定枚举放宽为可选字符串，允许绝对路径。
/// 已知的预设名原样保留，未知值退回 `null` 交给 `Preferences` 的默认值。
fn v1_to_v2(value: Value) -> Result<Value, MigrateError> {
    let Value::Object(mut object) = value else {
        return Err(MigrateError::Broken("配置顶层不是对象".into()));
    };
    let shell = match object.remove("shell") {
        Some(Value::String(name)) if !name.is_empty() => Value::String(name),
        Some(_) => Value::Null,
        None => return Ok(Value::Object(object)),
    };
    object.insert("shell".into(), shell);
    Ok(Value::Object(object))
}

/// 把版本号写回对象。迁移只处理字段，版本号统一在这里更新。
pub(crate) fn stamp(mut value: Value) -> Result<Value, MigrateError> {
    let Value::Object(ref mut object) = value else {
        return Err(MigrateError::Broken("配置顶层不是对象".into()));
    };
    upsert_version(object);
    Ok(value)
}

fn upsert_version(object: &mut Map<String, Value>) {
    object.insert("version".into(), Value::from(CURRENT_VERSION));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_v1_file_upgrades_to_the_current_version() {
        let value = serde_json::json!({"version": 1, "shell": "bash", "font_size": 13});
        let migrated = migrate(1, value).expect("v1 必须能升级");
        let migrated = stamp(migrated).expect("必须能写入版本号");
        assert_eq!(migrated["version"], CURRENT_VERSION);
        assert_eq!(migrated["shell"], "bash");
    }

    #[test]
    fn a_file_without_a_version_is_treated_as_v1() {
        let value = serde_json::json!({"shell": "fish"});
        let migrated = migrate(1, value).expect("缺版本按 v1 处理");
        assert_eq!(migrated["shell"], "fish");
    }

    #[test]
    fn a_newer_version_is_rejected_instead_of_downgraded() {
        assert!(matches!(
            migrate(CURRENT_VERSION + 1, serde_json::json!({})),
            Err(MigrateError::Unsupported(_))
        ));
    }

    #[test]
    fn version_zero_is_rejected() {
        assert!(matches!(
            migrate(0, serde_json::json!({})),
            Err(MigrateError::Broken(_))
        ));
    }

    #[test]
    fn a_non_object_document_is_rejected() {
        assert!(matches!(
            migrate(1, serde_json::json!([1, 2, 3])),
            Err(MigrateError::Broken(_))
        ));
    }
}
