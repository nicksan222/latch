//! Per-machine agent state in the config dir:
//! - `vaults.json`   known vaults and the active one
//! - `settings.json` machine settings (`protocol::Settings` minus the three
//!   vault-scoped flags, which live in the vault's config.toml)

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::core::{Error, Result};
use crate::protocol::{KnownVault, Settings, Theme};

pub const VAULTS_FILE: &str = "vaults.json";
pub const SETTINGS_FILE: &str = "settings.json";

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VaultsFile {
    #[serde(default)]
    pub active: Option<String>,
    #[serde(default)]
    pub vaults: Vec<VaultRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VaultRef {
    pub path: String,
    pub vault_id: String,
}

impl VaultsFile {
    pub fn load(config_dir: &Path) -> VaultsFile {
        read_json(&config_dir.join(VAULTS_FILE)).unwrap_or_default()
    }

    pub fn save(&self, config_dir: &Path) -> Result<()> {
        write_json(config_dir, VAULTS_FILE, self)
    }

    /// Register (or update) a vault and make it active.
    pub fn activate(&mut self, path: &Path, vault_id: &str) {
        let p = path.display().to_string();
        self.vaults.retain(|v| v.path != p);
        self.vaults.push(VaultRef {
            path: p.clone(),
            vault_id: vault_id.to_owned(),
        });
        self.active = Some(p);
    }

    pub fn active_path(&self) -> Option<PathBuf> {
        self.active.as_ref().map(PathBuf::from)
    }

    pub fn known(&self) -> Vec<KnownVault> {
        self.vaults
            .iter()
            .map(|v| KnownVault {
                path: v.path.clone(),
                vault_id: v.vault_id.clone(),
                active: self.active.as_deref() == Some(v.path.as_str()),
            })
            .collect()
    }
}

pub fn default_settings() -> Settings {
    Settings {
        auto_lock_secs: 900,
        clipboard_clear_secs: 30,
        reveal_hide_secs: 30,
        lock_on_screen_lock: true,
        tray_enabled: true,
        autostart: false,
        theme: Theme::System,
        global_shortcut: "CmdOrCtrl+Shift+L".into(),
        keep_browsers_open_on_lock: false,
        names_in_commits: false,
        auto_push: false,
        sessions_commit: false,
        git_name: None,
        git_email: None,
    }
}

/// Load settings; missing or malformed files fall back to defaults field by
/// field (unknown/missing keys take the default).
pub fn load_settings(config_dir: &Path) -> Settings {
    let def = serde_json::to_value(default_settings()).expect("serializable");
    let Some(stored) = read_json::<serde_json::Value>(&config_dir.join(SETTINGS_FILE)) else {
        return default_settings();
    };
    let mut merged = def;
    if let (Some(m), Some(s)) = (merged.as_object_mut(), stored.as_object()) {
        for (k, v) in s {
            if m.contains_key(k) {
                m.insert(k.clone(), v.clone());
            }
        }
    }
    serde_json::from_value(merged).unwrap_or_else(|_| default_settings())
}

pub fn save_settings(config_dir: &Path, s: &Settings) -> Result<()> {
    write_json(config_dir, SETTINGS_FILE, s)
}

pub fn validate_settings(s: &Settings) -> Result<()> {
    if s.clipboard_clear_secs > 24 * 3600 || s.reveal_hide_secs > 24 * 3600 {
        return Err(Error::InvalidParams("timeout too large".into()));
    }
    if s.auto_lock_secs > 7 * 24 * 3600 {
        return Err(Error::InvalidParams("autoLockSecs too large".into()));
    }
    Ok(())
}

fn read_json<T: for<'de> Deserialize<'de>>(p: &Path) -> Option<T> {
    let b = std::fs::read(p).ok()?;
    serde_json::from_slice(&b).ok()
}

fn write_json<T: Serialize>(dir: &Path, name: &str, v: &T) -> Result<()> {
    crate::core::paths::ensure_private_dir(dir).map_err(|e| Error::Io {
        context: format!("create {}", dir.display()),
        source: e,
    })?;
    let bytes = serde_json::to_vec_pretty(v).map_err(|e| Error::Internal(e.to_string()))?;
    crate::core::format::write_atomic(&dir.join(name), &bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vaults_roundtrip() {
        let t = tempfile::tempdir().unwrap();
        let mut v = VaultsFile::load(t.path());
        assert!(v.active.is_none());
        v.activate(Path::new("/a"), "id1");
        v.activate(Path::new("/b"), "id2");
        v.activate(Path::new("/a"), "id1");
        v.save(t.path()).unwrap();
        let v2 = VaultsFile::load(t.path());
        assert_eq!(v2, v);
        assert_eq!(v2.vaults.len(), 2);
        let k = v2.known();
        assert!(k.iter().find(|k| k.path == "/a").unwrap().active);
        assert!(!k.iter().find(|k| k.path == "/b").unwrap().active);
    }

    #[test]
    fn settings_partial_file() {
        let t = tempfile::tempdir().unwrap();
        assert_eq!(load_settings(t.path()), default_settings());
        std::fs::write(
            t.path().join(SETTINGS_FILE),
            r#"{"autoLockSecs": 5, "bogus": 1}"#,
        )
        .unwrap();
        let s = load_settings(t.path());
        assert_eq!(s.auto_lock_secs, 5);
        assert_eq!(s.clipboard_clear_secs, 30);
        save_settings(t.path(), &s).unwrap();
        assert_eq!(load_settings(t.path()), s);
    }
}
