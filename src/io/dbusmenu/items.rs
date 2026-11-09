// @Time    : 2026/11/5 12:36
// @Author  : fzf
// @FileName: items.rs
// @Software: Siskin

//! 菜单项树与布局序列化。协议字段在这里生成，服务层只负责加锁与转发。

use std::collections::HashMap;

use zbus::zvariant::{OwnedValue, Str, Value};

pub(crate) use super::ids;

pub(crate) type Properties = HashMap<String, OwnedValue>;
pub(crate) type LayoutNode = (i32, Properties, Vec<OwnedValue>);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MenuItem {
    id: i32,
    label: String,
    children: Vec<MenuItem>,
    separator: bool,
    toggle: Option<(&'static str, bool)>,
}

impl MenuItem {
    pub fn item(id: i32, label: impl Into<String>) -> Self {
        Self {
            id,
            label: label.into(),
            children: Vec::new(),
            separator: false,
            toggle: None,
        }
    }

    pub fn submenu(id: i32, label: &str, children: Vec<Self>) -> Self {
        Self {
            children,
            ..Self::item(id, label)
        }
    }

    pub fn separator(id: i32) -> Self {
        Self {
            separator: true,
            ..Self::item(id, "")
        }
    }

    pub fn checked(id: i32, label: &str, kind: &'static str, checked: bool) -> Self {
        Self {
            toggle: Some((kind, checked)),
            ..Self::item(id, label)
        }
    }

    pub fn find(&self, id: i32) -> Option<&Self> {
        if self.id == id {
            Some(self)
        } else {
            self.children.iter().find_map(|child| child.find(id))
        }
    }

    pub fn properties(&self, names: &[String]) -> Properties {
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

    pub fn layout(&self, depth: i32, names: &[String]) -> zbus::fdo::Result<LayoutNode> {
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

fn text_value(text: &str) -> OwnedValue {
    OwnedValue::from(Str::from(text.to_string()))
}
