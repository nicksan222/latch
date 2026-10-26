//! Bitwarden unencrypted JSON export.
//!
//! - type 1 login → `login` (uris, TOTP, custom fields)
//! - type 2 secure note → `note`
//! - type 3 card, type 4 identity → `note` (markdown body)
//! - type 5 SSH key → `ssh_key`
//!
//! Folders (personal vault) or the first collection (organization export)
//! map to profiles; nested folders `a/b` → profile `a`, tag `b`.

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;
use serde_json::Value;

use crate::core::import::{
    CustomFields, ImportedEntry, Parsed, parse_totp, read_text, split_folder,
};
use crate::core::{Error, Result};
use crate::protocol::{EntryPayload, LoginData, NoteData, SshKeyData};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Export {
    #[serde(default)]
    encrypted: bool,
    #[serde(default)]
    password_protected: bool,
    #[serde(default)]
    folders: Vec<Folder>,
    #[serde(default)]
    collections: Vec<Folder>,
    #[serde(default)]
    items: Vec<Item>,
}

#[derive(Deserialize)]
struct Folder {
    id: String,
    name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Item {
    #[serde(rename = "type")]
    kind: u8,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    notes: Option<String>,
    #[serde(default)]
    favorite: bool,
    #[serde(default)]
    folder_id: Option<String>,
    #[serde(default)]
    collection_ids: Option<Vec<String>>,
    #[serde(default)]
    fields: Option<Vec<Field>>,
    #[serde(default)]
    login: Option<Login>,
    #[serde(default)]
    card: Option<serde_json::Map<String, Value>>,
    #[serde(default)]
    identity: Option<serde_json::Map<String, Value>>,
    #[serde(default)]
    ssh_key: Option<SshKey>,
    #[serde(default)]
    deleted_date: Option<String>,
}

#[derive(Deserialize)]
struct Field {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    value: Option<String>,
    #[serde(rename = "type", default)]
    kind: u8,
}

#[derive(Deserialize)]
struct Login {
    #[serde(default)]
    username: Option<String>,
    #[serde(default)]
    password: Option<String>,
    #[serde(default)]
    totp: Option<String>,
    #[serde(default)]
    uris: Option<Vec<Uri>>,
}

#[derive(Deserialize)]
struct Uri {
    #[serde(default)]
    uri: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SshKey {
    #[serde(default)]
    private_key: Option<String>,
    #[serde(default)]
    public_key: Option<String>,
}

const CARD_FIELDS: &[(&str, &str)] = &[
    ("cardholderName", "Cardholder"),
    ("brand", "Brand"),
    ("number", "Number"),
    ("expMonth", "Expiry month"),
    ("expYear", "Expiry year"),
    ("code", "Security code"),
];

const IDENTITY_FIELDS: &[(&str, &str)] = &[
    ("title", "Title"),
    ("firstName", "First name"),
    ("middleName", "Middle name"),
    ("lastName", "Last name"),
    ("company", "Company"),
    ("email", "Email"),
    ("phone", "Phone"),
    ("username", "Username"),
    ("address1", "Address"),
    ("address2", "Address 2"),
    ("address3", "Address 3"),
    ("city", "City"),
    ("state", "State"),
    ("postalCode", "Postal code"),
    ("country", "Country"),
    ("ssn", "SSN"),
    ("passportNumber", "Passport number"),
    ("licenseNumber", "License number"),
];

fn markdown_fields(map: &serde_json::Map<String, Value>, spec: &[(&str, &str)]) -> String {
    let mut out = String::new();
    for (key, label) in spec {
        if let Some(v) = map.get(*key).and_then(value_str).filter(|v| !v.is_empty()) {
            out.push_str(&format!("- **{label}:** {v}\n"));
        }
    }
    out
}

fn value_str(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn parse(path: &Path, _passphrase: Option<&str>) -> Result<Parsed> {
    let text = read_text(path)?;
    // serde messages can quote input values: never surface them.
    let export: Export = serde_json::from_str(&text)
        .map_err(|e| Error::invalid(format!("malformed Bitwarden JSON (line {})", e.line())))?;
    if export.encrypted || export.password_protected {
        return Err(Error::invalid(
            "encrypted Bitwarden exports are not supported; export as \"JSON\" (unencrypted) and delete the file after importing",
        ));
    }
    let folders: HashMap<&str, &str> = export
        .folders
        .iter()
        .chain(export.collections.iter())
        .map(|f| (f.id.as_str(), f.name.as_str()))
        .collect();
    let mut out = Parsed::default();
    for item in &export.items {
        if item.deleted_date.is_some() {
            continue;
        }
        let name = item.name.clone().unwrap_or_default();
        let folder_name = item
            .folder_id
            .as_deref()
            .and_then(|id| folders.get(id))
            .or_else(|| {
                item.collection_ids
                    .as_ref()
                    .and_then(|c| c.first())
                    .and_then(|id| folders.get(id.as_str()))
            })
            .copied()
            .unwrap_or("");
        let (folder, tags) = split_folder(folder_name);
        let notes = item.notes.clone().unwrap_or_default();
        let mut custom = CustomFields::default();
        for f in item.fields.iter().flatten() {
            if f.kind == 3 {
                continue; // linked field: no value of its own
            }
            custom.push(
                f.name.as_deref().unwrap_or(""),
                f.value.as_deref().unwrap_or(""),
                f.kind == 1,
            );
        }
        let mut entry_notes = String::new();
        let payload = match item.kind {
            1 => {
                let l = item.login.as_ref();
                let totp_raw = l.and_then(|l| l.totp.as_deref()).unwrap_or("");
                let totp = parse_totp(totp_raw);
                if totp.is_none() && !totp_raw.trim().is_empty() {
                    custom.push("TOTP", totp_raw, true);
                    out.warnings.push(format!(
                        "`{name}`: unsupported TOTP format kept as a hidden custom field"
                    ));
                }
                entry_notes = notes.clone();
                EntryPayload::Login(LoginData {
                    username: l.and_then(|l| l.username.clone()).unwrap_or_default(),
                    password: Some(l.and_then(|l| l.password.clone()).unwrap_or_default()),
                    urls: l
                        .and_then(|l| l.uris.as_ref())
                        .into_iter()
                        .flatten()
                        .filter_map(|u| u.uri.clone())
                        .filter(|u| !u.trim().is_empty())
                        .collect(),
                    totp,
                    custom: std::mem::take(&mut custom.0),
                })
            }
            2 => EntryPayload::Note(NoteData {
                body: Some(with_custom(notes.clone(), &custom)),
            }),
            3 | 4 => {
                let (map, spec) = if item.kind == 3 {
                    (item.card.as_ref(), CARD_FIELDS)
                } else {
                    (item.identity.as_ref(), IDENTITY_FIELDS)
                };
                let mut body = map.map(|m| markdown_fields(m, spec)).unwrap_or_default();
                if !notes.is_empty() {
                    body.push_str(&format!("\n{notes}\n"));
                }
                EntryPayload::Note(NoteData {
                    body: Some(with_custom(body, &custom)),
                })
            }
            5 => {
                let k = item.ssh_key.as_ref();
                // Envelope notes are not a secret field: keep visible custom
                // fields only.
                if custom.0.iter().any(|c| c.hidden) {
                    out.warnings.push(format!(
                        "`{name}`: hidden custom fields on SSH keys are not imported"
                    ));
                }
                custom.0.retain(|c| !c.hidden);
                entry_notes = with_custom(notes.clone(), &custom);
                EntryPayload::SshKey(SshKeyData {
                    private_key: Some(k.and_then(|k| k.private_key.clone()).unwrap_or_default()),
                    public_key: k.and_then(|k| k.public_key.clone()).unwrap_or_default(),
                    passphrase: None,
                    comment: String::new(),
                })
            }
            other => {
                out.warnings.push(format!(
                    "skipped `{name}`: unsupported Bitwarden item type {other}"
                ));
                continue;
            }
        };
        out.entries.push(ImportedEntry {
            folder,
            name,
            tags,
            notes: entry_notes,
            favorite: item.favorite,
            payload,
            source_id: None,
        });
    }
    Ok(out)
}

/// Append custom fields to a markdown body (types without custom fields).
fn with_custom(mut body: String, custom: &CustomFields) -> String {
    if custom.0.is_empty() {
        return body;
    }
    if !body.is_empty() && !body.ends_with('\n') {
        body.push('\n');
    }
    body.push_str("\n**Fields**\n\n");
    for c in &custom.0 {
        body.push_str(&format!(
            "- **{}:** {}\n",
            c.key,
            c.value.as_deref().unwrap_or("")
        ));
    }
    body
}

import_kind! { format: Bitwarden, id: "bitwarden", parse: parse }
