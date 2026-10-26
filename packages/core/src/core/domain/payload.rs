use super::*;
use base64::Engine;
use serde_json::Value;
use sha2::{Digest, Sha256};

fn kind(p: &EntryPayload) -> Result<&'static secrets::SecretKind> {
    secrets::registered_kinds()
        .into_iter()
        .find(|k| k.entry_type == secrets::payload_type(p))
        .ok_or_else(|| Error::Internal("invalid secret kind registry".into()))
}
fn to_value(p: &EntryPayload) -> Result<Value> {
    serde_json::to_value(p).map_err(|_| Error::invalid("Invalid payload"))
}
fn from_value(v: Value) -> Result<EntryPayload> {
    serde_json::from_value(v).map_err(|_| Error::invalid("Invalid payload"))
}
fn pointer(path: &str) -> String {
    let path = if path == "content" {
        "contentBase64"
    } else {
        path
    };
    format!(
        "/{}",
        path.split('.')
            .map(|p| p.replace('~', "~0").replace('/', "~1"))
            .collect::<Vec<_>>()
            .join("/")
    )
}
fn at<'a>(data: &'a Value, path: &str) -> Option<&'a Value> {
    if let Some((prefix, key)) = path.split_once('.')
        && (prefix == "custom" || prefix == "vars")
    {
        return data[prefix]
            .as_array()?
            .iter()
            .find(|v| v["key"].as_str() == Some(key))
            .map(|v| &v["value"]);
    }
    data.pointer(&pointer(path))
}
fn set(data: &mut Value, path: &str, value: Value) {
    if let Some((prefix, key)) = path.split_once('.')
        && (prefix == "custom" || prefix == "vars")
    {
        if let Some(rows) = data[prefix].as_array_mut()
            && let Some(row) = rows.iter_mut().find(|v| v["key"].as_str() == Some(key))
        {
            row["value"] = value;
        }
    } else if let Some(target) = data.pointer_mut(&pointer(path)) {
        *target = value;
    }
}
fn paths(p: &EntryPayload, present_only: bool) -> Result<Vec<String>> {
    let value = to_value(p)?;
    let d = &value["data"];
    let mut out = Vec::new();
    for (path, _) in kind(p)?.wire_secrets {
        if let Some(prefix) = path.strip_suffix(".*") {
            if let Some(rows) = d[prefix].as_array() {
                for row in rows {
                    if row["hidden"] == true && (!present_only || !row["value"].is_null()) {
                        out.push(format!(
                            "{prefix}.{}",
                            row["key"].as_str().unwrap_or_default()
                        ));
                    }
                }
            }
        } else if at(d, path).is_some_and(|v| !present_only || !v.is_null()) {
            out.push((*path).into());
        }
    }
    Ok(out)
}
pub fn entry_defaults(input: EntryDefaultsInput) -> Result<EntryInput> {
    let k = secrets::registered_kinds()
        .into_iter()
        .find(|k| k.entry_type == input.entry_type)
        .ok_or_else(|| Error::invalid("Unknown entry type"))?;
    Ok(EntryInput {
        profile_id: input.profile_id,
        name: String::new(),
        tags: vec![],
        notes: String::new(),
        favorite: false,
        payload: (k.default_payload)(),
    })
}
pub fn database_url(data: DatabaseData) -> Result<String> {
    let body = secrets::body_from_payload(&EntryPayload::Database(data), None)?;
    let format::EntryBody::Database(d) = body else {
        return Err(Error::invalid("Expected database payload"));
    };
    Ok(secrets::database_url(&d))
}
pub fn secret_paths(p: EntryPayload) -> Result<Vec<String>> {
    paths(&p, true)
}
pub fn read_field(input: ReadFieldInput) -> Result<Option<String>> {
    if input.path == "url"
        && let EntryPayload::Database(d) = input.payload
    {
        return database_url(d).map(Some);
    }
    // Restrict reads to scalar fields and registered secret paths; never expose
    // serialized objects or convert a redacted null into an empty secret.
    let value = to_value(&input.payload)?;
    Ok(at(&value["data"], &input.path)
        .and_then(Value::as_str)
        .map(str::to_owned))
}
pub fn redact_payload(p: EntryPayload) -> Result<RedactedPayload> {
    let redacted = paths(&p, true)?;
    let all = paths(&p, false)?;
    let mut value = to_value(&p)?;
    for path in all {
        set(&mut value["data"], &path, Value::Null);
    }
    Ok(RedactedPayload {
        payload: from_value(value)?,
        redacted,
    })
}
pub fn apply_optional_clears(p: EntryPayload) -> Result<EntryPayload> {
    let mut value = to_value(&p)?;
    for (path, optional) in kind(&p)?.wire_secrets {
        if *optional && at(&value["data"], path).and_then(Value::as_str) == Some("") {
            if *path == "totp.secret" {
                value["data"]["totp"] = Value::Null;
            } else {
                set(&mut value["data"], path, Value::Null);
            }
        }
    }
    from_value(value)
}
pub fn merge_keep_on_none(input: PayloadMergeInput) -> Result<EntryPayload> {
    if secrets::payload_type(&input.stored) != secrets::payload_type(&input.input) {
        return apply_optional_clears(input.input);
    }
    // Drafts may contain masked or unfinished TOTP/file fields and therefore
    // cannot be decoded as valid stored documents. Reuse the storage merge
    // primitive and kind-owned path metadata without performing a vault write.
    let mut value = to_value(&input.input)?;
    let wire_stored = to_value(&input.stored)?;
    let mut candidates = paths(&input.input, false)?;
    for prefix in ["custom", "vars"] {
        if let Some(rows) = value["data"][prefix].as_array() {
            for row in rows {
                if let Some(key) = row["key"].as_str() {
                    candidates.push(format!("{prefix}.{key}"));
                }
            }
        }
    }
    let clears_totp = value["data"]["totp"]["secret"].as_str() == Some("");
    let metadata = kind(&input.input)?;
    for path in candidates {
        let optional = metadata
            .wire_secrets
            .iter()
            .any(|(p, optional)| *p == path && *optional);
        let merged = secrets::merge_secret(
            at(&value["data"], &path).and_then(Value::as_str),
            at(&wire_stored["data"], &path).and_then(Value::as_str),
            optional,
        )
        .map(str::to_owned);
        set(
            &mut value["data"],
            &path,
            merged.map_or(Value::Null, Value::String),
        );
    }
    if clears_totp {
        value["data"]["totp"] = Value::Null;
    }
    let wire_input = to_value(&input.input)?;
    if matches!(input.input, EntryPayload::File(_)) && wire_input["data"]["contentBase64"].is_null()
    {
        value["data"]["size"] = wire_stored["data"]["size"].clone();
    }
    apply_optional_clears(from_value(value)?)
}

fn bytes(size: u64) -> String {
    if size < 1024 {
        format!("{size} B")
    } else if size < 1024 * 1024 {
        format!("{:.1} KB", size as f64 / 1024.0)
    } else {
        format!("{:.1} MB", size as f64 / 1024.0 / 1024.0)
    }
}
pub fn flatten_entry(entry: EntryInput) -> Result<BTreeMap<String, String>> {
    let mut out = BTreeMap::from([
        ("name".into(), entry.name),
        ("tags".into(), entry.tags.join(", ")),
        ("notes".into(), entry.notes),
        (
            "favorite".into(),
            if entry.favorite { "yes" } else { "no" }.into(),
        ),
    ]);
    let value = to_value(&entry.payload)?;
    let d = &value["data"];
    if let EntryPayload::File(f) = &entry.payload {
        out.insert(
            "file".into(),
            format!(
                "{} · {} · {}",
                if f.filename.is_empty() {
                    "unnamed"
                } else {
                    &f.filename
                },
                if f.mime.is_empty() {
                    "application/octet-stream"
                } else {
                    &f.mime
                },
                bytes(f.size)
            ),
        );
        return Ok(out);
    }
    // The fields shown in conflict resolution are kind-owned metadata.
    for path in kind(&entry.payload)?.diff_fields {
        if let Some(prefix) = path.strip_suffix(".*") {
            if let Some(rows) = d[prefix].as_array() {
                for row in rows {
                    out.insert(
                        format!("{prefix}.{}", row["key"].as_str().unwrap_or_default()),
                        row["value"].as_str().unwrap_or_default().into(),
                    );
                }
            }
        } else {
            let v = at(d, path).unwrap_or(&Value::Null);
            let text = if *path == "urls" {
                v.as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(Value::as_str)
                            .collect::<Vec<_>>()
                            .join("\n")
                    })
                    .unwrap_or_default()
            } else if let Some(s) = v.as_str() {
                s.into()
            } else if v.is_null() {
                String::new()
            } else {
                v.to_string()
            };
            out.insert((*path).into(), text);
        }
    }
    Ok(out)
}
pub fn merge_entries(input: MergeEntriesInput) -> Result<EntryInput> {
    let mut base = input.mine;
    let other = input.theirs;
    let mut b = serde_json::to_value(&base).map_err(|_| Error::invalid("Invalid entry"))?;
    let o = serde_json::to_value(&other).map_err(|_| Error::invalid("Invalid entry"))?;
    let envelope = ["name", "tags", "notes", "favorite"];
    for key in &input.from_theirs {
        if envelope.contains(&key.as_str()) {
            b[key] = o[key].clone();
        }
    }
    if secrets::payload_type(&base.payload) != secrets::payload_type(&other.payload) {
        if input
            .from_theirs
            .iter()
            .any(|k| !envelope.contains(&k.as_str()))
        {
            b["payload"] = o["payload"].clone();
        }
    } else {
        for key in &input.from_theirs {
            if envelope.contains(&key.as_str()) {
                continue;
            }
            if key == "file" && matches!(base.payload, EntryPayload::File(_)) {
                b["payload"]["data"] = o["payload"]["data"].clone();
            } else if key == "totp.secret" && matches!(base.payload, EntryPayload::Login(_)) {
                b["payload"]["data"]["totp"] = o["payload"]["data"]["totp"].clone();
            } else if let Some((prefix, k)) = key.split_once('.')
                && ["custom", "vars"].contains(&prefix)
            {
                if let Some(rows) = b["payload"]["data"][prefix].as_array_mut() {
                    let theirs = o["payload"]["data"][prefix]
                        .as_array()
                        .and_then(|a| a.iter().find(|v| v["key"].as_str() == Some(k)));
                    let idx = rows.iter().position(|v| v["key"].as_str() == Some(k));
                    // Custom fields append selected values; env vars retain position.
                    if prefix == "vars"
                        && let (Some(i), Some(v)) = (idx, theirs)
                    {
                        rows[i] = v.clone();
                    } else {
                        if let Some(i) = idx {
                            rows.remove(i);
                        }
                        if let Some(v) = theirs {
                            rows.push(v.clone());
                        }
                    }
                }
            } else if let Some(v) = o["payload"]["data"].get(key) {
                b["payload"]["data"][key] = v.clone();
            }
        }
    }
    base = serde_json::from_value(b).map_err(|_| Error::invalid("Invalid merged entry"))?;
    Ok(base)
}
pub fn file_digest(entry: Option<EntryInput>) -> Result<Option<String>> {
    let Some(EntryInput {
        payload: EntryPayload::File(f),
        ..
    }) = entry
    else {
        return Ok(None);
    };
    let Some(content) = f.content_base64 else {
        return Ok(None);
    };
    let content = zeroize::Zeroizing::new(content);
    // Match browser atob for drafts (unpadded base64 and ASCII whitespace),
    // while actual stored file validation remains strict in its strategy.
    let normalized = zeroize::Zeroizing::new(
        content
            .chars()
            .filter(|c| !matches!(c, ' ' | '\t' | '\n' | '\r' | '\u{000c}'))
            .collect::<String>(),
    );
    let decoder = base64::engine::GeneralPurpose::new(
        &base64::alphabet::STANDARD,
        base64::engine::GeneralPurposeConfig::new()
            .with_decode_padding_mode(base64::engine::DecodePaddingMode::Indifferent)
            .with_decode_allow_trailing_bits(true),
    );
    let data = zeroize::Zeroizing::new(
        decoder
            .decode(normalized.as_bytes())
            .unwrap_or_else(|_| content.as_bytes().to_vec()),
    );
    Ok(Some(format!("{:x}", Sha256::digest(&*data))))
}
pub fn diff_rows(input: DiffRowsInput) -> Result<Vec<DomainDiffRow>> {
    let md = file_digest(input.mine.clone())?;
    let td = file_digest(input.theirs.clone())?;
    let mine = input
        .mine
        .map(flatten_entry)
        .transpose()?
        .unwrap_or_default();
    let theirs = input
        .theirs
        .map(flatten_entry)
        .transpose()?
        .unwrap_or_default();
    let keys: std::collections::BTreeSet<_> = mine.keys().chain(theirs.keys()).cloned().collect();
    Ok(keys
        .into_iter()
        .map(|field| {
            let a = mine.get(&field).cloned().unwrap_or_default();
            let b = theirs.get(&field).cloned().unwrap_or_default();
            let content_differs = field == "file" && md != td;
            DomainDiffRow {
                field,
                differs: a != b || content_differs,
                mine: a,
                theirs: b,
                content_differs,
            }
        })
        .collect())
}

/// Synchronous UI metadata. This is generated, never maintained in TypeScript.
pub fn catalog() -> Result<DomainCatalog> {
    let mut kinds = secrets::registered_kinds();
    kinds.sort_by_key(|k| k.ui_order);
    Ok(DomainCatalog {
        entry_types: kinds
            .iter()
            .map(|k| DomainEntryTypeMetadata {
                r#type: k.entry_type,
                label: k.label.into(),
                primary_copy_field: k.primary_copy_field.map(str::to_owned),
                default_payload: (k.default_payload)(),
            })
            .collect(),
        default_browser_settings: default_browser_settings(())?,
        default_settings: crate::agent::state::default_settings(),
        default_totp: TotpConfig {
            secret: Some(String::new()),
            algorithm: TotpAlgorithm::Sha1,
            digits: 6,
            period: 30,
            issuer: None,
        },
    })
}

#[cfg(test)]
#[test]
fn export_bindings_domain_catalog() {
    let base = std::env::var_os("LATCH_CATALOG_EXPORT_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("bindings"));
    let path = base.join("domain-catalog.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        path,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&catalog().unwrap()).unwrap()
        ),
    )
    .unwrap();
}
