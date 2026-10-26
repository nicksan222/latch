//! `pass` (password-store) directory: every `*.gpg` file is decrypted by
//! shelling out to `gpg --decrypt` (skipped with a warning when gpg is not
//! installed). First line = password; `login:`/`user:`/`username:`/`email:`,
//! `url:` and `otpauth://` lines make it a `login`, otherwise a `secret`
//! (remaining lines → notes). `a/b/c.gpg` → profile `a`, name `b - c`.
//!
//! The gpg binary can be overridden with `LATCH_GPG`.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use zeroize::Zeroizing;

use crate::core::import::{CustomFields, ImportedEntry, Parsed, parse_totp};
use crate::core::{Error, Result};
use crate::protocol::{EntryPayload, LoginData, SecretData};

fn gpg_binary() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("LATCH_GPG").filter(|v| !v.is_empty()) {
        let p = PathBuf::from(p);
        // A configured but missing binary behaves like "gpg not installed".
        return p.is_file().then_some(p);
    }
    which::which("gpg").or_else(|_| which::which("gpg2")).ok()
}

/// Files accepted in one store.
const MAX_FILES: usize = 20_000;
/// Directory nesting accepted.
const MAX_DEPTH: usize = 32;
/// Decrypted size of one file (entries are limited to 1 MiB).
const MAX_DECRYPTED_BYTES: u64 = 1024 * 1024;
/// Decrypted size of all files together.
const MAX_TOTAL_BYTES: u64 = 256 * 1024 * 1024;

fn too_many() -> std::io::Error {
    std::io::Error::other("the store has too many files or is nested too deeply")
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    collect_at(dir, out, 0)
}

fn collect_at(dir: &Path, out: &mut Vec<PathBuf>, depth: usize) -> std::io::Result<()> {
    if depth > MAX_DEPTH {
        return Err(too_many());
    }
    let mut items: Vec<_> = std::fs::read_dir(dir)?.flatten().collect();
    items.sort_by_key(|e| e.file_name());
    for e in items {
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue; // .git, .gpg-id, .extensions
        }
        let ft = e.file_type()?;
        if ft.is_dir() {
            collect_at(&e.path(), out, depth + 1)?;
        } else if ft.is_file() && name.ends_with(".gpg") {
            if out.len() >= MAX_FILES {
                return Err(too_many());
            }
            out.push(e.path());
        }
    }
    Ok(())
}

/// Decrypt one file, reading at most `MAX_DECRYPTED_BYTES` of gpg output
/// (larger output kills gpg and skips the file).
fn decrypt(gpg: &Path, file: &Path) -> Option<Zeroizing<String>> {
    let mut child = Command::new(gpg)
        .args(["--quiet", "--batch", "--yes", "--decrypt"])
        .arg(file)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut stdout = Zeroizing::new(Vec::new());
    let read = child
        .stdout
        .take()
        .map(|o| o.take(MAX_DECRYPTED_BYTES + 1).read_to_end(&mut stdout));
    if stdout.len() as u64 > MAX_DECRYPTED_BYTES || !matches!(read, Some(Ok(_))) {
        let _ = child.kill();
        let _ = child.wait();
        return None;
    }
    let status = child.wait().ok()?;
    if !status.success() {
        return None;
    }
    std::str::from_utf8(&stdout)
        .ok()
        .map(|s| Zeroizing::new(s.to_owned()))
}

/// Parse decrypted `pass` content into a payload and envelope notes.
pub(crate) fn parse_content(text: &str) -> (EntryPayload, String) {
    let mut lines = text.lines();
    let password = lines.next().unwrap_or("").to_owned();
    let mut username = String::new();
    let mut urls = vec![];
    let mut totp = None;
    let mut custom = CustomFields::default();
    let mut notes: Vec<&str> = vec![];
    for line in lines {
        let t = line.trim();
        if t.to_ascii_lowercase().starts_with("otpauth://") {
            if totp.is_none() {
                totp = parse_totp(t);
            }
            if totp.is_some() {
                continue;
            }
        }
        if let Some((k, v)) = t.split_once(':') {
            let key = k.trim();
            let lk = key.to_ascii_lowercase();
            let v = v.trim();
            match lk.as_str() {
                "login" | "user" | "username" if username.is_empty() => {
                    username = v.to_owned();
                    continue;
                }
                "email" if username.is_empty() => {
                    username = v.to_owned();
                    continue;
                }
                "url" | "website" | "site" => {
                    urls.push(v.to_owned());
                    continue;
                }
                _ => {}
            }
            let simple_key =
                !key.is_empty() && key.len() <= 40 && !key.contains(' ') && !lk.starts_with("http");
            if simple_key && !v.is_empty() {
                custom.push(key, v, true);
                continue;
            }
        }
        notes.push(line);
    }
    let notes = notes.join("\n").trim().to_owned();
    if !username.is_empty() || !urls.is_empty() || totp.is_some() {
        (
            EntryPayload::Login(LoginData {
                username,
                password: Some(password),
                urls,
                totp,
                custom: custom.0,
            }),
            notes,
        )
    } else {
        // Secrets have no custom fields; keep them as a hidden value suffix
        // rather than in the (non-secret) notes.
        let mut value = password;
        for c in &custom.0 {
            value.push_str(&format!(
                "\n{}: {}",
                c.key,
                c.value.as_deref().unwrap_or("")
            ));
        }
        (
            EntryPayload::Secret(SecretData { value: Some(value) }),
            notes,
        )
    }
}

fn parse(path: &Path, _passphrase: Option<&str>) -> Result<Parsed> {
    parse_with(path, gpg_binary())
}

fn parse_with(path: &Path, gpg: Option<PathBuf>) -> Result<Parsed> {
    if !path.is_dir() {
        return Err(Error::invalid(format!(
            "{} is not a pass store directory",
            path.display()
        )));
    }
    let mut files = vec![];
    collect(path, &mut files).map_err(|e| Error::io(format!("read {}", path.display()), e))?;
    let mut out = Parsed::default();
    let mut total: u64 = 0;
    if files.is_empty() {
        out.warnings.push("no .gpg files found".into());
        return Ok(out);
    }
    let Some(gpg) = gpg else {
        out.warnings.push(format!(
            "gpg not found; {} encrypted file(s) were skipped",
            files.len()
        ));
        return Ok(out);
    };
    for f in files {
        let rel = f.strip_prefix(path).unwrap_or(&f);
        let mut parts: Vec<String> = rel
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect();
        if let Some(last) = parts.last_mut() {
            *last = last.trim_end_matches(".gpg").to_owned();
        }
        let display = parts.join("/");
        let Some(text) = decrypt(&gpg, &f) else {
            out.warnings.push(format!(
                "could not decrypt `{display}` (or it is larger than 1 MiB)"
            ));
            continue;
        };
        total += text.len() as u64;
        if total > MAX_TOTAL_BYTES {
            return Err(Error::invalid(
                "the pass store is too large to import at once",
            ));
        }
        let (folder, name) = if parts.len() > 1 {
            (Some(parts[0].clone()), parts[1..].join(" - "))
        } else {
            (None, parts.join(""))
        };
        let (payload, notes) = parse_content(&text);
        out.entries.push(ImportedEntry {
            folder,
            name,
            tags: vec![],
            notes,
            favorite: false,
            payload,
            source_id: None,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_content() {
        let (p, notes) = parse_content(
            "hunter2\nlogin: alice\nurl: https://example.com\notpauth://totp/x?secret=JBSWY3DPEHPK3PXP\npin: 1234\nfree text here\n",
        );
        let EntryPayload::Login(l) = p else { panic!() };
        assert_eq!(l.username, "alice");
        assert_eq!(l.password.as_deref(), Some("hunter2"));
        assert_eq!(l.urls, ["https://example.com"]);
        assert!(l.totp.is_some());
        assert_eq!(l.custom.len(), 1);
        assert_eq!(notes, "free text here");
    }

    #[test]
    fn missing_gpg_warns() {
        let store = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/import/pass-store");
        let p = parse_with(&store, None).unwrap();
        assert!(p.entries.is_empty());
        assert!(p.warnings[0].contains("gpg not found"), "{:?}", p.warnings);
    }

    #[test]
    fn secret_content() {
        let (p, notes) = parse_content("s3cret\nsome comment line\n");
        let EntryPayload::Secret(s) = p else { panic!() };
        assert_eq!(s.value.as_deref(), Some("s3cret"));
        assert_eq!(notes, "some comment line");
    }
}

import_kind! { format: Pass, id: "pass", parse: parse }
