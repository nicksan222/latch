//! Vault lifecycle and authenticated persistence boundary.
//!
//! [`Vault`] locates a locked vault; [`UnlockedVault`] owns its decrypted state,
//! keys, integrity guard and write transactions. Sibling features (profiles,
//! secrets, environment, history, sync and recipients) operate on this handle
//! through its existing guarded storage API; they do not own another vault.
//!
//! Implementation responsibilities are private: `lifecycle` handles credentials
//! and open/unlock/reload/lock, `init` creates encrypted state, `keys` authenticates
//! rotations, `integrity` verifies trust and signed state, `storage` reads/writes
//! encrypted documents, and `transactions` guards and commits mutations.
//! No protocol or on-disk representation is defined by these implementation modules.

mod init;
mod integrity;
mod keys;
mod lifecycle;
mod storage;
mod transactions;

pub(crate) use init::check_passphrase_strength;
pub(crate) use integrity::{STATE_FILES, empty_manifest, list_vault_files, load_state, snapshot};
pub(crate) use keys::{Keys, PendingKeys, decrypt_vaultkey, encrypt_vaultkey, resolve_keys};
pub use lifecycle::Credentials;
pub(crate) use lifecycle::UnlockSecret;

use std::path::{Path, PathBuf};

use crate::core::error::Result;
use crate::core::format::{ConfigDoc, KeychainDoc, ManifestDoc, RecipientDoc, now};
use crate::core::git::Git;
use crate::core::trust::{self, Consent, TrustInfo, TrustState};
use crate::protocol::{RecipientInfo, VaultStatus};

/// A vault on disk, locked.
#[derive(Debug, Clone)]
pub struct Vault {
    root: PathBuf,
    config: ConfigDoc,
}

fn last_sync_at(git: &Git) -> Option<String> {
    let p = git.git_path("latch-last-sync").ok()?;
    std::fs::read_to_string(p)
        .ok()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
}

pub(crate) fn record_sync(git: &Git) {
    if let Ok(p) = git.git_path("latch-last-sync") {
        let _ = std::fs::write(p, now());
    }
}

fn status_for(root: &Path, config: &ConfigDoc, locked: bool) -> VaultStatus {
    let git = Git::new(root);
    let repo = git.is_repo();
    let st = if repo { git.status().ok() } else { None };
    VaultStatus {
        initialized: true,
        locked,
        vault_id: Some(config.vault.id.clone()),
        path: Some(root.display().to_string()),
        recipients: config
            .recipients
            .iter()
            .map(RecipientDoc::to_info)
            .collect(),
        dirty: st.as_ref().is_some_and(|s| s.dirty),
        ahead: st.as_ref().map_or(0, |s| s.ahead),
        behind: st.as_ref().map_or(0, |s| s.behind),
        has_remote: repo && git.remote_url().is_some(),
        last_sync_at: if repo { last_sync_at(&git) } else { None },
        locks_in_secs: None,
        trust: None,
        integrity_problems: vec![],
        rotation_pending: false,
        memory_locked: None,
    }
}

/// Status for a path that holds no vault.
pub fn uninitialized_status(path: Option<&Path>) -> VaultStatus {
    VaultStatus {
        initialized: false,
        locked: true,
        vault_id: None,
        path: path.map(|p| p.display().to_string()),
        recipients: vec![],
        dirty: false,
        ahead: 0,
        behind: 0,
        has_remote: false,
        last_sync_at: None,
        locks_in_secs: None,
        trust: None,
        integrity_problems: vec![],
        rotation_pending: false,
        memory_locked: None,
    }
}

impl Vault {
    pub fn path(&self) -> &Path {
        &self.root
    }
    pub fn id(&self) -> &str {
        &self.config.vault.id
    }
    pub fn config(&self) -> &ConfigDoc {
        &self.config
    }
    pub fn recipients(&self) -> Vec<RecipientInfo> {
        self.config
            .recipients
            .iter()
            .map(RecipientDoc::to_info)
            .collect()
    }
    pub fn status(&self) -> VaultStatus {
        status_for(&self.root, &self.config, true)
    }
}

/// An unlocked vault. Not `Clone`; dropping or [`UnlockedVault::lock`]
/// zeroizes key material.
pub struct UnlockedVault {
    pub(crate) root: PathBuf,
    pub(crate) git: Git,
    pub(crate) config: ConfigDoc,
    pub(crate) keys: Keys,
    pub(crate) secret: UnlockSecret,
    pub(crate) manifest: ManifestDoc,
    /// Hash of the manifest ciphertext we last read or wrote.
    pub(crate) manifest_ct_hash: String,
    /// Integrity problems found; non-empty blocks data access and writes.
    pub(crate) problems: Vec<IntegrityProblem>,
    /// Verified keychain (on disk).
    pub(crate) chain: KeychainDoc,
    pub(crate) trust_state: TrustState,
    /// Hashes of [`STATE_FILES`] as last verified or written by us; any
    /// unexplained change triggers a full re-verification before writes.
    pub(crate) verified: Vec<String>,
    /// Upstream commit authenticated during the current sync and its
    /// config.toml bytes (see `sync.rs`).
    pub(crate) verified_onto: Option<(String, Vec<u8>)>,
    /// Local consent of this clone (see [`Consent`]).
    pub(crate) consent: Consent,
    /// Last config hash recorded as trusted for this clone.
    pub(crate) remembered_config: String,
}

/// See [`UnlockedVault::begin_txn`].
pub(crate) struct WriteTxn {
    manifest: ManifestDoc,
    ct_hash: String,
    manifest_bytes: Option<Vec<u8>>,
    sig_bytes: Option<Vec<u8>>,
    /// Vault-relative files created by the batch.
    pub created_files: Vec<String>,
    /// Vault-relative dirs created by the batch (removed recursively).
    pub created_dirs: Vec<String>,
}

/// A detected integrity problem (no secret content).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrityProblem {
    pub code: &'static str,
    pub message: String,
    pub path: Option<String>,
}

impl std::fmt::Debug for UnlockedVault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UnlockedVault")
            .field("root", &self.root)
            .field("vault_id", &self.config.vault.id)
            .finish_non_exhaustive()
    }
}

impl UnlockedVault {
    pub fn path(&self) -> &Path {
        &self.root
    }
    pub fn id(&self) -> &str {
        &self.config.vault.id
    }
    pub fn config(&self) -> &ConfigDoc {
        &self.config
    }
    pub fn git(&self) -> &Git {
        &self.git
    }
    pub fn manifest(&self) -> &ManifestDoc {
        &self.manifest
    }
    /// Integrity problems that currently block access (empty when healthy).
    pub fn integrity_problems(&self) -> &[IntegrityProblem] {
        &self.problems
    }
    pub fn status(&self) -> VaultStatus {
        status_for(&self.root, &self.config, false)
    }
    pub fn recipients(&self) -> Vec<RecipientInfo> {
        self.config
            .recipients
            .iter()
            .map(RecipientDoc::to_info)
            .collect()
    }

    /// Trust status: `FirstUse` right after a trust-on-first-use unlock
    /// (show the fingerprint for out-of-band comparison), `Pinned`, or
    /// `Stale` (this clone lags behind a rotation pinned on this machine).
    pub fn trust_info(&self) -> TrustInfo {
        TrustInfo {
            state: self.trust_state,
            fingerprint: self.fingerprint(),
            key_seq: self.chain.head().seq,
        }
    }

    /// Stable vault fingerprint (genesis key of the verified keychain).
    pub fn fingerprint(&self) -> String {
        trust::fingerprint(&self.chain.records[0])
    }

    /// A key rotation was interrupted; `doctor --fix` or `rotate` finishes it.
    pub fn rotation_pending(&self) -> bool {
        self.keys.next.is_some() || self.keys.on_disk_pending
    }

    /// This clone's local consent for disclosure-affecting settings.
    pub fn local_consent(&self) -> Consent {
        self.consent
    }

    /// Record local consent (per clone, in the trust file outside the repo).
    /// Does not change config.toml.
    pub fn set_local_consent(&mut self, c: Consent) -> Result<()> {
        trust::set_consent(&self.config.vault.id, &self.root, c)?;
        self.consent = c;
        Ok(())
    }

    /// Settings in effect on this machine: each config.toml flag AND this
    /// clone's local consent. Use these (not `config()`) to decide whether
    /// to put names in commits, auto-push or commit session archives.
    pub fn effective_settings(&self) -> Consent {
        Consent {
            names_in_commits: self.config.sync.names_in_commits && self.consent.names_in_commits,
            auto_push: self.config.sync.auto_push && self.consent.auto_push,
            sessions_commit: self.config.sessions.commit && self.consent.sessions_commit,
        }
    }

    /// Update `[sync]`/`[sessions]` flags in config.toml (signed: the
    /// manifest is re-signed) and commit. This is the local user's explicit
    /// choice, so it also records the same values as this clone's local
    /// consent (see [`UnlockedVault::effective_settings`]).
    pub fn set_vault_settings(
        &mut self,
        names_in_commits: bool,
        auto_push: bool,
        sessions_commit: bool,
    ) -> Result<()> {
        self.ensure_writable()?;
        self.set_local_consent(Consent {
            names_in_commits,
            auto_push,
            sessions_commit,
        })?;
        if self.config.sync.names_in_commits == names_in_commits
            && self.config.sync.auto_push == auto_push
            && self.config.sessions.commit == sessions_commit
        {
            return Ok(());
        }
        let backup = self.config.clone();
        self.config.sync.names_in_commits = names_in_commits;
        self.config.sync.auto_push = auto_push;
        self.config.sessions.commit = sessions_commit;
        if let Err(e) = self.write_config().and_then(|_| self.save_manifest()) {
            self.config = backup;
            return Err(e);
        }
        self.commit("vault: update settings")?;
        Ok(())
    }
}

/// Test helper: a fresh vault with weak KDF params in `dir`.
#[cfg(test)]
pub(crate) fn test_vault(dir: &Path) -> UnlockedVault {
    Vault::init(&crate::protocol::InitParams {
        path: dir.join("vault").display().to_string(),
        passphrase: "correct horse battery staple".into(),
        kdf: Some(crate::core::crypto::WEAK_KDF),
        git_name: Some("Test".into()),
        git_email: Some("test@example.com".into()),
        remote_url: None,
        recovery_kit: false,
    })
    .expect("init")
    .0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_send_sync<T: Send + Sync>() {}

    #[test]
    fn handles_are_send_sync() {
        assert_send_sync::<UnlockedVault>();
        assert_send_sync::<Vault>();
    }
}
