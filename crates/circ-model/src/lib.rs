//! Circuitaria document model: the single source of truth for project files.
//! Pure data + structural helpers; no I/O, no coordinates-aware logic.

#![recursion_limit = "512"]
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub mod builder;
pub mod catalog;
pub use catalog::*;

pub const SCHEMA_VERSION: u32 = 1;
/// Hard limits applied when loading untrusted files.
pub const MAX_FILE_BYTES: usize = 64 << 20;
pub const MAX_COMPONENTS: usize = 2_000_000;

pub type Id = u64;
pub type Params = BTreeMap<String, Value>;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub schema_version: u32,
    pub name: String,
    /// Id of the root module.
    pub main: String,
    pub modules: BTreeMap<String, Module>,
    #[serde(default)]
    pub next_id: Id,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub meta: Params,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Module {
    pub name: String,
    #[serde(default)]
    pub color: String,
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub revision: u32,
    pub components: Vec<Component>,
    pub wires: Vec<Wire>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Component {
    pub id: Id,
    pub kind: String,
    pub x: i32,
    pub y: i32,
    #[serde(default)]
    pub rot: u8,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: Params,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PinRef {
    pub c: Id,
    pub p: String,
}

/// A connection between two pins. By convention `a` is the source side, but
/// connectivity is undirected: nets are the connected components of wires.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Wire {
    pub id: Id,
    pub a: PinRef,
    pub b: PinRef,
}

impl Project {
    pub fn new(name: &str) -> Self {
        let mut modules = BTreeMap::new();
        modules.insert("main".to_string(), Module { name: "main".into(), ..Default::default() });
        Project { schema_version: SCHEMA_VERSION, name: name.into(), main: "main".into(), modules, next_id: 1, meta: Params::new() }
    }

    /// Defensive load: size limits, schema migration, structural limits.
    pub fn from_json(s: &str) -> Result<Project, String> {
        if s.len() > MAX_FILE_BYTES {
            return Err("file_too_large".into());
        }
        let v: Value = serde_json::from_str(s).map_err(|e| format!("invalid_json: {e}"))?;
        let v = migrate(v)?;
        let p: Project = serde_json::from_value(v).map_err(|e| format!("invalid_project: {e}"))?;
        let n: usize = p.modules.values().map(|m| m.components.len()).sum();
        if n > MAX_COMPONENTS {
            return Err("too_many_components".into());
        }
        if !p.modules.contains_key(&p.main) {
            return Err("missing_main_module".into());
        }
        Ok(p)
    }

    /// Deterministic (sorted keys, stable field order) pretty JSON, friendly to diff/git.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("project serializes")
    }
}

/// Schema migrations. Each step upgrades `schemaVersion` by one.
fn migrate(v: Value) -> Result<Value, String> {
    let ver = v.get("schemaVersion").and_then(Value::as_u64).ok_or("missing_schema_version")? as u32;
    if ver > SCHEMA_VERSION {
        return Err(format!("newer_schema: {ver}"));
    }
    if ver == 0 {
        return Err("unsupported_schema: 0".into());
    }
    // v1 is the first released schema: nothing to migrate yet.
    Ok(v)
}

/// Follows a hierarchical instance path ("12/45/") from the root module.
pub fn module_at<'a>(p: &'a Project, path: &str) -> Option<(&'a str, &'a Module)> {
    let mut id: &str = &p.main;
    let mut m = p.modules.get(id)?;
    for seg in path.split('/').filter(|s| !s.is_empty()) {
        let cid: Id = seg.parse().ok()?;
        let c = m.components.iter().find(|c| c.id == cid && c.kind == "ic")?;
        id = c.params.get("module")?.as_str()?;
        m = p.modules.get(id)?;
    }
    Some((id, m))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_is_identical() {
        let mut p = Project::new("t");
        let mut b = builder::MB::new("main");
        let s = b.add("switch", serde_json::json!({}));
        let l = b.add("led", serde_json::json!({"color": "red"}));
        b.wire((s, "y"), (l, "a"));
        p.modules.insert("main".into(), b.finish());
        let j = p.to_json();
        let q = Project::from_json(&j).unwrap();
        assert_eq!(p, q);
        assert_eq!(j, q.to_json());
    }

    #[test]
    fn rejects_bad_files() {
        assert!(Project::from_json("{").is_err());
        assert!(Project::from_json("{}").is_err());
        assert!(Project::from_json(r#"{"schemaVersion":99,"name":"x","main":"main","modules":{}}"#).is_err());
        assert!(Project::from_json(r#"{"schemaVersion":1,"name":"x","main":"nope","modules":{}}"#).is_err());
    }
}
