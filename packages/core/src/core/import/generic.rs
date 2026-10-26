//! Generic JSON / YAML key-value files → `env` entries.
//!
//! - A flat map → one env entry named after the file.
//! - Nested: top-level scalars → one entry named after the file; every
//!   top-level map/list → its own entry named after the key, with nested
//!   keys flattened with `_` (`db: {primary: {url}}` → entry `db`, var
//!   `primary_url`). Invalid characters in keys become `_`.
//! - SOPS-encrypted files are rejected (decrypt with `sops -d` first).
//! - A plaintext Latch JSON export (`"format": "latch-archive"`) is imported
//!   as such (profiles and all entry types).

use std::path::Path;

use serde_json::Value;

use super::{ImportedEntry, Parsed, file_stem, read_text};
use crate::core::{Error, Result};
use crate::protocol::{EntryPayload, EnvData, EnvVar};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    Json,
    Yaml,
}

const SOPS_MSG: &str = "this file is SOPS-encrypted; decrypt it first (e.g. `sops -d secrets.yaml > plain.yaml`), import the plaintext copy, then delete it";

fn is_sops(v: &Value) -> bool {
    match v {
        Value::Object(m) => m.get("sops").is_some_and(Value::is_object) || m.values().any(is_sops),
        Value::Array(a) => a.iter().any(is_sops),
        Value::String(s) => s.starts_with("ENC[") && s.ends_with(']'),
        _ => false,
    }
}

fn scalar(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Null => Some(String::new()),
        _ => None,
    }
}

fn sanitize_key(k: &str) -> String {
    let mut s: String = k
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if !s.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_') {
        s.insert(0, '_');
    }
    s
}

fn flatten(prefix: &str, v: &Value, out: &mut Vec<(String, String)>) {
    let join = |k: &str| {
        if prefix.is_empty() {
            k.to_owned()
        } else {
            format!("{prefix}_{k}")
        }
    };
    match v {
        Value::Object(m) => {
            for (k, c) in m {
                flatten(&join(k), c, out);
            }
        }
        Value::Array(a) => {
            for (i, c) in a.iter().enumerate() {
                flatten(&join(&i.to_string()), c, out);
            }
        }
        other => {
            let key = sanitize_key(prefix);
            let val = scalar(other).unwrap_or_default();
            // Later duplicates win (keys must be unique within an entry).
            out.retain(|(k, _)| *k != key);
            out.push((key, val));
        }
    }
}

fn env_entry(name: &str, pairs: Vec<(String, String)>) -> ImportedEntry {
    ImportedEntry::new(
        name,
        EntryPayload::Env(EnvData {
            vars: pairs
                .into_iter()
                .map(|(key, value)| EnvVar {
                    key,
                    value: Some(value),
                    hidden: true,
                    comment: None,
                })
                .collect(),
        }),
    )
}

pub(super) fn parse(path: &Path, kind: Kind) -> Result<Parsed> {
    let text = read_text(path)?;
    let value: Value = match kind {
        Kind::Json => serde_json::from_str(&text)
            .map_err(|e| Error::invalid(format!("malformed JSON (line {})", e.line())))?,
        Kind::Yaml => serde_yaml::from_str(&text).map_err(|e| {
            Error::invalid(match e.location() {
                Some(l) => format!("malformed YAML (line {})", l.line()),
                None => "malformed YAML".into(),
            })
        })?,
    };
    if value.get("format").and_then(Value::as_str) == Some(crate::core::export::ARCHIVE_FORMAT) {
        let doc: crate::core::export::ArchiveDoc =
            serde_json::from_value(value).map_err(|_| Error::invalid("malformed Latch export"))?;
        crate::core::export::validate_archive(&doc)?;
        return Ok(super::from_archive(doc));
    }
    if is_sops(&value) {
        return Err(Error::invalid(SOPS_MSG));
    }
    let Value::Object(map) = value else {
        return Err(Error::invalid("expected a key/value map at the top level"));
    };
    let stem = file_stem(path);
    let mut out = Parsed::default();
    let mut top = vec![];
    let mut nested = vec![];
    for (k, v) in &map {
        if scalar(v).is_some() {
            flatten(k, v, &mut top);
        } else {
            let mut pairs = vec![];
            flatten("", v, &mut pairs);
            if pairs.is_empty() {
                out.warnings.push(format!("`{k}` is empty; skipped"));
            } else {
                nested.push(env_entry(k, pairs));
            }
        }
    }
    if !top.is_empty() {
        out.entries.push(env_entry(&stem, top));
    }
    out.entries.extend(nested);
    if out.entries.is_empty() {
        out.warnings.push("no values found".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn sops_detected() {
        assert!(is_sops(&json!({"a": 1, "sops": {"mac": "x"}})));
        assert!(is_sops(&json!({"a": "ENC[AES256_GCM,data:abc]"})));
        assert!(!is_sops(&json!({"a": "b"})));
    }

    #[test]
    fn flatten_nested() {
        let mut out = vec![];
        flatten(
            "",
            &json!({"primary": {"url": "x", "port": 5432}, "list": ["a", "b"], "bad key": true}),
            &mut out,
        );
        let keys: Vec<_> = out.iter().map(|(k, _)| k.as_str()).collect();
        assert!(keys.contains(&"primary_url"));
        assert!(keys.contains(&"primary_port"));
        assert!(keys.contains(&"list_1"));
        assert!(keys.contains(&"bad_key"));
        assert_eq!(sanitize_key("1abc"), "_1abc");
    }
}
