//! One-way, workspace-scoped native Firefox login snapshots. No credential is
//! returned through RPC, argv, environment variables or the runtime install.
use std::collections::BTreeMap;
use std::path::Path;
use std::time::{Duration, Instant};

use serde::Serialize;
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

use crate::core::format::EntryBody;
use crate::core::{Error, Result, UnlockedVault, ids};
use crate::protocol::{EntryType, ListEntriesParams};

pub const PENDING: &str = "latch-credentials.json";
pub const ACK: &str = "latch-credentials-result.json";

#[derive(Serialize, Zeroize, ZeroizeOnDrop)]
struct Credential {
    id: String,
    origin: String,
    username: String,
    password: String,
}

#[derive(Serialize, Zeroize, ZeroizeOnDrop)]
struct Snapshot {
    version: u8,
    generation: String,
    credentials: Vec<Credential>,
}

/// An origin never contains a username, password, path, query or fragment.
fn origin(value: &str) -> Option<String> {
    let url = url::Url::parse(value).ok()?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return None;
    }
    Some(url.origin().ascii_serialization())
}

fn snapshot(vault: &UnlockedVault, profile_id: &str) -> Result<Snapshot> {
    vault.get_profile(profile_id)?;
    let mut credentials = BTreeMap::new();
    for entry in vault.list_entries(&ListEntriesParams {
        profile_id: Some(profile_id.into()),
        entry_type: Some(EntryType::Login),
        query: None,
    })? {
        let doc = vault.read_entry_doc(&entry.id)?;
        let EntryBody::Login(login) = &doc.body else {
            continue;
        };
        let Some(password) = login.password.as_ref().filter(|p| !p.is_empty()) else {
            continue;
        };
        for url in &login.urls {
            let Some(origin) = origin(url) else { continue };
            let key = (origin.clone(), login.username.clone());
            if let Some(existing) = credentials.get(&key) {
                let existing: &Credential = existing;
                if existing.password != *password {
                    return Err(Error::Conflict(
                        "workspace has conflicting browser logins for the same origin and username"
                            .into(),
                    ));
                }
                continue;
            }
            credentials.insert(
                key,
                Credential {
                    id: doc.id.clone(),
                    origin,
                    username: login.username.clone(),
                    password: password.clone(),
                },
            );
        }
    }
    if credentials.len() > 10000 {
        return Err(Error::invalid("too many workspace browser logins"));
    }
    Ok(Snapshot {
        version: 1,
        generation: ids::new_id(),
        credentials: credentials.into_values().collect(),
    })
}

/// Install static trusted startup code only into an app-owned Firefox runtime.
/// Returns whether the bundle was changed (macOS needs re-signing then).
pub fn install_bootstrap(executable: &Path) -> Result<bool> {
    install_bootstrap_with(executable, super::runtime::finalize_bootstrap)
}

fn install_bootstrap_with(
    executable: &Path,
    finalize: impl FnOnce(&Path) -> Result<()>,
) -> Result<bool> {
    let root = super::runtime::config_root(executable);
    // Playwright packages platform resources differently. Replace its existing
    // selector, rather than loading the automation defaults or adding a second one.
    let preference_path = if executable
        .parent()
        .and_then(|p| p.file_name())
        .is_some_and(|name| name == "MacOS")
    {
        "defaults/pref/00-playwright-prefs.js"
    } else {
        "browser/defaults/preferences/00-playwright-prefs.js"
    };
    let files = [
        (
            preference_path,
            include_bytes!("firefox-prefs.js").as_slice(),
        ),
        ("latch.cfg", include_bytes!("firefox.cfg").as_slice()),
    ];
    let lock = std::fs::File::options()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(root.join(".latch-bootstrap.lock"))
        .map_err(|e| Error::io("open browser bootstrap lock", e))?;
    fs2::FileExt::lock_exclusive(&lock)
        .map_err(|e| Error::io("lock browser bootstrap installation", e))?;
    let dirty = root.join(".latch-bootstrap-unsigned");
    let mut changed = dirty.exists();
    for (relative, bytes) in files {
        let target = root.join(relative);
        if std::fs::read(&target).ok().as_deref() != Some(bytes) {
            super::write_file_in(
                &root,
                Path::new(".latch-bootstrap-unsigned"),
                b"configuration requires signing",
            )?;
            changed = true;
            super::write_file_in(&root, Path::new(relative), bytes)?;
        }
    }
    if changed {
        finalize(executable)?;
        std::fs::remove_file(&dirty)
            .map_err(|e| Error::io("complete browser bootstrap signing", e))?;
    }
    Ok(changed)
}

/// Publish atomically so Firefox never observes partially written plaintext.
/// `profile` must already be a private session directory outside the vault.
pub fn prepare(vault: &UnlockedVault, profile_id: &str, profile: &Path) -> Result<String> {
    let canonical = profile
        .canonicalize()
        .map_err(|e| Error::io("locate browser profile", e))?;
    let root = vault
        .path()
        .canonicalize()
        .map_err(|e| Error::io("locate vault", e))?;
    if canonical.starts_with(root) {
        return Err(Error::Denied(
            "browser credentials cannot be written beneath a vault".into(),
        ));
    }
    crate::core::paths::check_secure_dir(&canonical)
        .map_err(|e| Error::io("check browser profile permissions", e))?;
    let snapshot = snapshot(vault, profile_id)?;
    let bytes = Zeroizing::new(
        serde_json::to_vec(&snapshot)
            .map_err(|_| Error::Internal("encode browser login snapshot".into()))?,
    );
    cleanup(profile);
    let temporary = super::write_file_in(profile, Path::new("latch-credentials.pending"), &bytes)?;
    std::fs::rename(&temporary, profile.join(PENDING))
        .map_err(|e| Error::io("publish browser login snapshot", e))?;
    Ok(snapshot.generation.clone())
}

/// The browser acknowledges successful native password-store synchronization.
/// Do not report launch success if native import failed or never ran.
pub fn wait(profile: &Path, generation: &str, timeout: Duration) -> Result<()> {
    let start = Instant::now();
    loop {
        if let Some(result) = super::read_json(&profile.join(ACK))
            && result.get("generation").and_then(|x| x.as_str()) == Some(generation)
        {
            cleanup(profile);
            return if result.get("ok").and_then(|x| x.as_bool()) == Some(true) {
                Ok(())
            } else {
                Err(Error::Conflict("Firefox could not synchronize workspace logins; check for conflicting browser-owned logins".into()))
            };
        }
        if start.elapsed() >= timeout {
            cleanup(profile);
            return Err(Error::Conflict(
                "Firefox did not acknowledge workspace logins; close it and retry".into(),
            ));
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

pub fn cleanup(profile: &Path) {
    for file in [PENDING, ACK, "latch-credentials.pending"] {
        super::remove_any(&profile.join(file));
    }
}

#[cfg(test)]
#[path = "tests/credentials.rs"]
mod tests;
