//! Authenticate key history, manifests, configuration and persisted ciphertext.

use std::path::{Path, PathBuf};

use super::{IntegrityProblem, Keys, UnlockSecret, UnlockedVault, decrypt_vaultkey, resolve_keys};
use crate::core::crypto;
use crate::core::error::{Error, Result};
use crate::core::format::*;
use crate::core::git::Git;
use crate::core::trust;

/// Config, keychain and keys after every trust check passed.
pub(crate) struct LoadedState {
    pub config: ConfigDoc,
    pub chain: KeychainDoc,
    pub keys: Keys,
    pub first_use: bool,
    pub relation: trust::Relation,
}

/// The per-clone marker `.git/latch-vault-id` (local, never pushed): a
/// clone whose config suddenly names another vault id is refused instead of
/// being trusted on first use under the new id.
pub(super) fn clone_marker(git: &Git) -> Option<PathBuf> {
    git.is_repo()
        .then(|| git.git_path("latch-vault-id").ok())
        .flatten()
}

/// Read version, config, keychain and `vaultkey.age` and run every trust
/// check (keychain signatures, pin, clone marker, key consistency).
pub(crate) fn load_state(root: &Path, secret: &UnlockSecret) -> Result<LoadedState> {
    read_version(root)?;
    let config = ConfigDoc::read(root)?;
    let id = config.vault.id.clone();
    trust::trust_path(&id)?; // validates the id
    if let Some(m) = clone_marker(&Git::new(root))
        && let Ok(s) = std::fs::read_to_string(&m)
        && !s.trim().is_empty()
        && s.trim() != id
    {
        return Err(Error::integrity(format!(
            "config.toml names vault `{id}` but this clone belongs to vault `{}`",
            s.trim()
        )));
    }
    let chain = KeychainDoc::read(root)?;
    trust::verify_chain(&chain, &id)?;
    let pinned = trust::pinned(&id)?;
    let first_use = pinned.is_empty();
    let relation = trust::relation(&pinned, &chain.records)?;
    let doc = decrypt_vaultkey(&read_file(&root.join(VAULTKEY_FILE))?, secret)?;
    let keys = resolve_keys(&config, &chain, &doc)?;
    Ok(LoadedState {
        config,
        chain,
        keys,
        first_use,
        relation,
    })
}

/// Files whose content is authenticated as a whole (see `verified`).
pub(crate) const STATE_FILES: [&str; 6] = [
    VERSION_FILE,
    CONFIG_FILE,
    KEYCHAIN_FILE,
    VAULTKEY_FILE,
    MANIFEST_FILE,
    MANIFEST_SIG_FILE,
];

fn file_state(root: &Path, rel: &str) -> String {
    std::fs::read(root.join(rel))
        .map(|b| crypto::hash(&b))
        .unwrap_or_else(|_| "missing".into())
}

pub(crate) fn snapshot(root: &Path) -> Vec<String> {
    STATE_FILES.iter().map(|f| file_state(root, f)).collect()
}

pub(crate) fn empty_manifest(config: &ConfigDoc) -> ManifestDoc {
    ManifestDoc {
        version: FORMAT_VERSION,
        updated_at: now(),
        config_hash: config.security_hash(),
        profiles: vec![],
    }
}

impl UnlockedVault {
    /// Verify manifest signature/binding and every file hash. When healthy,
    /// record the verified state and advance the trust pin.
    pub(crate) fn load_and_verify(&mut self) {
        self.problems.clear();
        match self.read_manifest_from_disk() {
            Ok((m, h)) => {
                self.manifest = m;
                self.manifest_ct_hash = h;
                let problems = self.verify_files();
                self.problems.extend(problems);
            }
            Err(p) => {
                self.problems.push(p);
                self.manifest = empty_manifest(&self.config);
                self.manifest_ct_hash.clear();
            }
        }
        self.verified = snapshot(&self.root);
        if self.problems.is_empty() {
            self.commit_trust();
        }
    }

    /// Pin the verified key history, mark this clone with its vault id and
    /// remember the verified config (best effort: failures are logged).
    pub(crate) fn commit_trust(&mut self) {
        if let Err(e) = trust::pin(&self.config.vault.id, &self.chain.records) {
            tracing::warn!("cannot update the trust pin: {e}");
        }
        if let Some(m) = clone_marker(&self.git)
            && std::fs::read_to_string(&m)
                .map(|s| s.trim().to_owned())
                .ok()
                .as_deref()
                != Some(self.config.vault.id.as_str())
        {
            let _ = std::fs::write(&m, format!("{}\n", self.config.vault.id));
        }
        self.remember_config();
    }

    /// Re-hash some of [`STATE_FILES`] after we wrote them ourselves.
    pub(crate) fn mark_verified(&mut self, files: &[&str]) {
        if self.verified.len() != STATE_FILES.len() {
            self.verified = snapshot(&self.root);
            return;
        }
        for f in files {
            if let Some(i) = STATE_FILES.iter().position(|x| x == f) {
                self.verified[i] = file_state(&self.root, f);
            }
        }
    }

    fn trusted_config_file(&self) -> Option<PathBuf> {
        if !self.git.is_repo() {
            return None;
        }
        self.git.git_path("latch-trusted-config").ok()
    }

    /// Record a config hash as authenticated for this clone (signed by us
    /// or verified under a trusted key). Doctor only re-signs configs whose
    /// hash was recorded (keeps the 8 most recent).
    pub(crate) fn remember_config_hash(&mut self, hash: &str) {
        if self.remembered_config == hash {
            return;
        }
        if let Some(p) = self.trusted_config_file() {
            let text = std::fs::read_to_string(&p).unwrap_or_default();
            let mut list: Vec<&str> = text
                .lines()
                .filter(|l| !l.is_empty() && *l != hash)
                .collect();
            list.push(hash);
            let skip = list.len().saturating_sub(8);
            let out: String = list[skip..].iter().map(|l| format!("{l}\n")).collect();
            let _ = std::fs::write(&p, out);
        }
        self.remembered_config = hash.to_owned();
    }

    pub(crate) fn remember_config(&mut self) {
        let h = self.config.security_hash();
        self.remember_config_hash(&h);
    }

    /// Whether this clone authenticated a config with this hash before.
    pub(crate) fn config_trusted(&self, hash: &str) -> bool {
        self.trusted_config_file()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .is_some_and(|t| t.lines().any(|l| l == hash))
    }

    /// Remember the new config hash, then write config.toml.
    pub(crate) fn write_config(&mut self) -> Result<()> {
        let h = self.config.security_hash();
        self.remember_config_hash(&h);
        self.config.write(&self.root)?;
        self.mark_verified(&[CONFIG_FILE]);
        Ok(())
    }

    /// Read, verify and decrypt the on-disk manifest.
    pub(crate) fn read_manifest_from_disk(
        &self,
    ) -> std::result::Result<(ManifestDoc, String), IntegrityProblem> {
        let prob = |code: &'static str, msg: &str, path: &str| IntegrityProblem {
            code,
            message: msg.to_owned(),
            path: Some(path.to_owned()),
        };
        let ct = std::fs::read(self.root.join(MANIFEST_FILE))
            .map_err(|_| prob("manifest_missing", "manifest.age is missing", MANIFEST_FILE))?;
        let sig = std::fs::read_to_string(self.root.join(MANIFEST_SIG_FILE)).map_err(|_| {
            prob(
                "manifest_signature",
                "manifest.sig is missing",
                MANIFEST_SIG_FILE,
            )
        })?;
        let vk = self.keys.signing.verifying_key();
        if !crypto::verify(&vk, &ct, &sig) {
            return Err(prob(
                "manifest_signature",
                "vault manifest not signed by this vault",
                MANIFEST_SIG_FILE,
            ));
        }
        let pt = crypto::decrypt_with(&self.keys.all_identities(), &ct).map_err(|_| {
            prob(
                "manifest_decrypt",
                "manifest.age cannot be decrypted",
                MANIFEST_FILE,
            )
        })?;
        let m = ManifestDoc::from_json(&pt).map_err(|_| {
            prob(
                "manifest_decrypt",
                "manifest.age is malformed",
                MANIFEST_FILE,
            )
        })?;
        if m.config_hash != self.config.security_hash() {
            return Err(prob(
                "config_mismatch",
                "config.toml (KDF, vault key, recipients or settings) does not match the signed manifest",
                CONFIG_FILE,
            ));
        }
        Ok((m, crypto::hash(&ct)))
    }

    /// Compare manifest hashes with files; report missing/orphan files.
    pub(crate) fn verify_files(&self) -> Vec<IntegrityProblem> {
        let mut out = vec![];
        let mut expected = std::collections::BTreeSet::new();
        for p in &self.manifest.profiles {
            let rel = profile_file(&p.id);
            expected.insert(rel.clone());
            check_hash(&self.root, &rel, &p.hash, &mut out);
            for e in &p.entries {
                let rel = entry_file(&p.id, &e.id);
                expected.insert(rel.clone());
                check_hash(&self.root, &rel, &e.hash, &mut out);
            }
        }
        // Stray non-layout files are the plaintext scanner's business.
        for rel in list_vault_files(&self.root) {
            let layout = parse_entry_path(&rel).is_some() || parse_profile_path(&rel).is_some();
            if layout && !expected.contains(&rel) {
                out.push(IntegrityProblem {
                    code: "orphan_file",
                    message: "file is not listed in the signed manifest".into(),
                    path: Some(rel),
                });
            }
        }
        out
    }

    pub(crate) fn check_ok(&self) -> Result<()> {
        if let Some(p) = self.problems.first() {
            return Err(Error::integrity(format!(
                "{} (run doctor; {} problem(s))",
                p.message,
                self.problems.len()
            )));
        }
        Ok(())
    }
}

fn check_hash(root: &Path, rel: &str, expected: &str, out: &mut Vec<IntegrityProblem>) {
    match std::fs::read(root.join(rel)) {
        Ok(ct) => {
            if crypto::hash(&ct) != expected {
                out.push(IntegrityProblem {
                    code: "hash_mismatch",
                    message: "file does not match the signed manifest".into(),
                    path: Some(rel.to_owned()),
                });
            }
        }
        Err(_) => out.push(IntegrityProblem {
            code: "missing_file",
            message: "file listed in the manifest is missing".into(),
            path: Some(rel.to_owned()),
        }),
    }
}

/// Every profile/entry file under `profiles/` (relative paths).
pub(crate) fn list_vault_files(root: &Path) -> Vec<String> {
    let mut out = vec![];
    let Ok(rd) = std::fs::read_dir(root.join(PROFILES_DIR)) else {
        return out;
    };
    for p in rd.flatten() {
        let pid = p.file_name().to_string_lossy().to_string();
        if !p.path().is_dir() {
            out.push(format!("{PROFILES_DIR}/{pid}"));
            continue;
        }
        if let Ok(rd2) = std::fs::read_dir(p.path()) {
            for f in rd2.flatten() {
                let name = f.file_name().to_string_lossy().to_string();
                if name == "entries" && f.path().is_dir() {
                    if let Ok(rd3) = std::fs::read_dir(f.path()) {
                        for e in rd3.flatten() {
                            out.push(format!(
                                "{PROFILES_DIR}/{pid}/entries/{}",
                                e.file_name().to_string_lossy()
                            ));
                        }
                    }
                } else {
                    out.push(format!("{PROFILES_DIR}/{pid}/{name}"));
                }
            }
        }
    }
    out.sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_signature_blocks_the_handle() {
        let dir = tempfile::tempdir().unwrap();
        let mut vault = crate::core::vault::test_vault(dir.path());
        std::fs::write(vault.root.join(MANIFEST_SIG_FILE), "invalid").unwrap();
        vault.load_and_verify();
        assert_eq!(vault.integrity_problems()[0].code, "manifest_signature");
        assert!(matches!(vault.check_ok(), Err(Error::Integrity(_))));
    }
}
