//! Guard writes, commit only vault paths and roll back uncommitted batches.

use super::{UnlockedVault, WriteTxn, snapshot};
use crate::core::error::{Error, Result};
use crate::core::format::*;

impl UnlockedVault {
    /// Before writing: healthy, a git repo, no sync in progress, and the
    /// authenticated state files (version, config, keychain, vault key,
    /// manifest, signature) are exactly what we last verified or wrote. Any
    /// unexplained state change triggers a reload. Check every profile and
    /// entry ciphertext against the signed manifest on each write: those files
    /// can change without changing the state-file snapshot. Failures block the
    /// handle until `doctor` repairs the vault (never silently overwritten).
    pub(crate) fn ensure_writable(&mut self) -> Result<()> {
        self.check_ok()?;
        if !self.git.is_repo() {
            return Err(Error::Git(
                "vault is not a git repository; writes are disabled".into(),
            ));
        }
        if self.git.rebase_in_progress() {
            return Err(Error::Conflict(
                "a sync is in progress; resolve conflicts first".into(),
            ));
        }
        if snapshot(&self.root) != self.verified {
            self.reload()?;
        }
        self.check_ok()?;
        self.problems = self.verify_files();
        self.check_ok()
    }

    /// Commit every changed vault path (see [`is_vault_path`]); nothing else
    /// in the work tree or the user's index is ever included. Pushes when
    /// `auto_push` is in effect (config AND local consent; best effort).
    pub(crate) fn commit(&self, message: &str) -> Result<Option<String>> {
        let changed: Vec<String> = self
            .git
            .changed_paths()?
            .into_iter()
            .filter(|p| is_vault_path(p))
            .collect();
        let c = self.commit_vault_paths(message, &changed, &[])?;
        if c.is_some()
            && self.effective_settings().auto_push
            && self.git.remote_url().is_some()
            && let Err(e) = self.git.push(&self.git.current_branch())
        {
            tracing::warn!("auto-push failed: {e}");
        }
        Ok(c)
    }

    /// Commit `update` (work-tree content; missing = deleted) and `remove`
    /// with a temporary index. Every updated file is scanned like `doctor`
    /// (valid age header for `.age`, allowed plaintext otherwise) and every
    /// path of the resulting tree must be a vault path; violations abort
    /// before any git object is created.
    pub(crate) fn commit_vault_paths(
        &self,
        message: &str,
        update: &[String],
        remove: &[String],
    ) -> Result<Option<String>> {
        let public = crate::core::doctor::public_tokens(&self.root);
        for p in update {
            if !is_vault_path(p) {
                return Err(Error::integrity(format!(
                    "refusing to commit `{p}`: not a vault file"
                )));
            }
            if let Ok(content) = std::fs::read(self.root.join(p))
                && let Some(i) = crate::core::doctor::check_file(p, &content, &public)
                    .into_iter()
                    .find(|i| i.severity == crate::protocol::Severity::Error)
            {
                return Err(Error::integrity(format!(
                    "refusing to commit `{p}`: {}",
                    i.message
                )));
            }
        }
        let check = |paths: &[String]| -> Result<()> {
            match paths.iter().find(|p| !is_vault_path(p)) {
                Some(p) => Err(Error::integrity(format!(
                    "the commit would contain `{p}`, which is not a vault file (run doctor --fix to untrack it)"
                ))),
                None => Ok(()),
            }
        };
        self.git.commit_paths(message, update, remove, &check)
    }

    /// Label for commit messages: name when `names_in_commits` is in effect
    /// (config AND local consent), else id.
    pub(crate) fn label(&self, id: &str, name: &str) -> String {
        if self.effective_settings().names_in_commits {
            name.to_owned()
        } else {
            id.to_owned()
        }
    }

    pub(crate) fn profile_label(&self, pid: &str) -> String {
        let name = self
            .manifest
            .profile(pid)
            .map(|p| p.name.clone())
            .unwrap_or_default();
        self.label(pid, &name)
    }

    /// Start a batch of uncommitted writes that must apply all-or-nothing
    /// (import). Record created paths in the returned [`WriteTxn`]; on
    /// failure [`UnlockedVault::rollback`] restores the work tree and the
    /// in-memory state.
    pub(crate) fn begin_txn(&self) -> WriteTxn {
        WriteTxn {
            manifest: self.manifest.clone(),
            ct_hash: self.manifest_ct_hash.clone(),
            manifest_bytes: std::fs::read(self.root.join(MANIFEST_FILE)).ok(),
            sig_bytes: std::fs::read(self.root.join(MANIFEST_SIG_FILE)).ok(),
            created_files: vec![],
            created_dirs: vec![],
        }
    }

    /// Undo a failed batch: remove what it created and restore the signed
    /// manifest bytes (so the repository is exactly as before).
    pub(crate) fn rollback(&mut self, txn: WriteTxn) {
        for f in &txn.created_files {
            let _ = std::fs::remove_file(self.root.join(f));
        }
        for d in &txn.created_dirs {
            let _ = std::fs::remove_dir_all(self.root.join(d));
        }
        for (rel, bytes) in [
            (MANIFEST_FILE, &txn.manifest_bytes),
            (MANIFEST_SIG_FILE, &txn.sig_bytes),
        ] {
            match bytes {
                Some(b) => {
                    if let Err(e) = write_atomic(&self.root.join(rel), b) {
                        tracing::error!("rollback: cannot restore {rel}: {e}");
                    }
                }
                None => {
                    let _ = std::fs::remove_file(self.root.join(rel));
                }
            }
        }
        self.manifest = txn.manifest;
        self.manifest_ct_hash = txn.ct_hash;
        self.mark_verified(&[MANIFEST_FILE, MANIFEST_SIG_FILE]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rollback_restores_authenticated_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let mut vault = crate::core::vault::test_vault(dir.path());
        let before = snapshot(vault.path());
        let updated_at = vault.manifest.updated_at.clone();
        let txn = vault.begin_txn();
        vault.manifest.updated_at = "uncommitted".into();
        vault.save_manifest().unwrap();
        assert_ne!(snapshot(vault.path()), before);
        vault.rollback(txn);
        assert_eq!(snapshot(vault.path()), before);
        assert_eq!(vault.manifest.updated_at, updated_at);
        vault.reload().unwrap();
        assert!(vault.integrity_problems().is_empty());
    }
}
