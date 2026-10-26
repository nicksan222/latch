//! CSV importers: 1Password CSV and Chrome/Firefox password exports.
//!
//! Columns are matched by (case-insensitive) header name, so both
//! 1Password 7/8 layouts and Chrome (`name,url,username,password,note`) /
//! Firefox (`url,username,password,httpRealm,…`) exports work.

use std::path::Path;

use super::{CustomFields, ImportedEntry, Parsed, host_of, parse_totp, read_text};
use crate::core::{Error, Result};
use crate::protocol::{ApiKeyData, EntryPayload, LoginData, NoteData};

struct Table {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
}

impl Table {
    fn col(&self, names: &[&str]) -> Option<usize> {
        names
            .iter()
            .find_map(|n| self.headers.iter().position(|h| h == n))
    }
}

fn read_table(path: &Path) -> Result<Table> {
    let text = read_text(path)?;
    let mut r = csv::ReaderBuilder::new()
        .flexible(true)
        .has_headers(true)
        .from_reader(text.as_bytes());
    let headers: Vec<String> = r
        .headers()
        .map_err(|_| Error::invalid("malformed CSV header"))?
        .iter()
        .map(|h| h.trim().to_ascii_lowercase())
        .collect();
    let mut rows = vec![];
    for (i, rec) in r.records().enumerate() {
        // csv errors can quote field contents: report the record number only.
        let rec = rec.map_err(|_| Error::invalid(format!("malformed CSV at record {}", i + 1)))?;
        rows.push(rec.iter().map(str::to_owned).collect());
    }
    Ok(Table { headers, rows })
}

fn get(row: &[String], idx: Option<usize>) -> String {
    idx.and_then(|i| row.get(i))
        .map(|s| s.trim().to_owned())
        .unwrap_or_default()
}

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "y"
    )
}

fn name_from_url(url: &str) -> String {
    host_of(url).unwrap_or_default()
}

pub(super) fn parse_onepassword(path: &Path) -> Result<Parsed> {
    let t = read_table(path)?;
    let title = t.col(&["title", "name"]);
    let url = t.col(&["url", "website", "urls", "login url", "location"]);
    let user = t.col(&["username", "login username", "user"]);
    let pass = t.col(&["password", "login password"]);
    let otp = t.col(&["otpauth", "one-time password", "totp", "otp"]);
    let notes = t.col(&["notes", "notesplain", "note"]);
    let tags = t.col(&["tags"]);
    let fav = t.col(&["favorite"]);
    let archived = t.col(&["archived"]);
    let ty = t.col(&["type", "category"]);
    let credential = t.col(&["credential"]);
    if title.is_none() && url.is_none() {
        return Err(Error::invalid(
            "not a 1Password CSV export (no Title or Url column)",
        ));
    }
    let known: Vec<usize> = [
        title, url, user, pass, otp, notes, tags, fav, archived, ty, credential,
    ]
    .into_iter()
    .flatten()
    .collect();
    let mut out = Parsed::default();
    for row in &t.rows {
        let url_s = get(row, url);
        let mut name = get(row, title);
        if name.is_empty() {
            name = name_from_url(&url_s);
        }
        let username = get(row, user);
        let password = get(row, pass);
        let otp_s = get(row, otp);
        let note = get(row, notes);
        let category = get(row, ty).to_ascii_lowercase();
        let mut tag_list: Vec<String> = get(row, tags)
            .split([',', ';'])
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .collect();
        if truthy(&get(row, archived)) {
            tag_list.push("archived".into());
        }
        let mut custom = CustomFields::default();
        for (i, v) in row.iter().enumerate() {
            if known.contains(&i) || v.trim().is_empty() {
                continue;
            }
            let key = t.headers.get(i).cloned().unwrap_or_default();
            custom.push(&key, v.trim(), true);
        }
        let urls: Vec<String> = url_s
            .lines()
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .collect();
        let mut notes_out = note.clone();
        let payload = if category.contains("api") {
            let key = {
                let c = get(row, credential);
                if c.is_empty() { password.clone() } else { c }
            };
            EntryPayload::ApiKey(ApiKeyData {
                key: Some(key),
                secret: None,
                endpoint: urls.first().cloned(),
                expires_at: None,
            })
        } else if username.is_empty()
            && password.is_empty()
            && urls.is_empty()
            && otp_s.is_empty()
            && custom.0.is_empty()
        {
            notes_out.clear();
            EntryPayload::Note(NoteData { body: Some(note) })
        } else {
            let totp = parse_totp(&otp_s);
            if totp.is_none() && !otp_s.is_empty() {
                custom.push("TOTP", &otp_s, true);
            }
            EntryPayload::Login(LoginData {
                username,
                password: Some(password),
                urls,
                totp,
                custom: custom.0,
            })
        };
        out.entries.push(ImportedEntry {
            folder: None,
            name,
            tags: tag_list,
            notes: notes_out,
            favorite: truthy(&get(row, fav)),
            payload,
            source_id: None,
        });
    }
    Ok(out)
}

pub(super) fn parse_browser(path: &Path) -> Result<Parsed> {
    let t = read_table(path)?;
    let name = t.col(&["name", "title"]);
    let url = t.col(&["url", "origin", "hostname", "origin_url"]);
    let user = t.col(&["username", "username_value", "login"]);
    let pass = t.col(&["password", "password_value"]);
    let note = t.col(&["note", "notes"]);
    if url.is_none() || pass.is_none() {
        return Err(Error::invalid(
            "not a browser password export (needs url and password columns)",
        ));
    }
    let mut out = Parsed::default();
    for row in &t.rows {
        let url_s = get(row, url);
        let password = get(row, pass);
        let username = get(row, user);
        if url_s.is_empty() && password.is_empty() && username.is_empty() {
            continue;
        }
        let mut n = get(row, name);
        if n.is_empty() {
            n = name_from_url(&url_s);
        }
        if n.is_empty() {
            n = url_s.clone();
        }
        out.entries.push(ImportedEntry {
            folder: None,
            name: n,
            tags: vec![],
            notes: get(row, note),
            favorite: false,
            payload: EntryPayload::Login(LoginData {
                username,
                password: Some(password),
                urls: if url_s.is_empty() {
                    vec![]
                } else {
                    vec![url_s]
                },
                totp: None,
                custom: vec![],
            }),
            source_id: None,
        });
    }
    Ok(out)
}
