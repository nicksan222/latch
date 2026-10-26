//! Recipients (who can decrypt `vaultkey.age`), key rotation, passphrase
//! change, and blob encryption helpers for session archives.
//!
//! - add/remove re-encrypt only `vaultkey.age` (+ config, re-signed manifest).
//! - rotate generates a new vault key and signing key, re-encrypts every
//!   profile, entry, manifest and session archive in one commit. Old vault
//!   keys are kept inside `vaultkey.age` (`previous_vault_keys`) so current
//!   recipients can still read git history. The new signing key is
//!   introduced by a keychain record signed by the old one (see `trust`).
//!
//! Rotation is crash safe (FORMAT.md §4.2): everything is validated first
//! (all documents and all session archives must decrypt, else nothing is
//! touched); then `vaultkey.age` is durably rewritten holding BOTH the
//! current keys and the pending next keys (with their signed keychain
//! record); only then are archives and documents re-encrypted (temp file +
//! rename each), the keychain/config switched, the manifest re-signed and
//! the pending block dropped. An interrupted rotation always leaves every
//! file decryptable; `doctor --fix` (or `rotate` again) finishes it.

use std::path::PathBuf;

use secrecy::SecretString;
use zeroize::Zeroizing;

use super::crypto::{self, SecretKey32};
use super::error::{Error, Result};
use super::format::*;
use super::paths;
use super::trust;
use super::vault::{
    PendingKeys, UnlockSecret, UnlockedVault, check_passphrase_strength, encrypt_vaultkey,
};
use crate::protocol::{
    AddRecipientParams, ChangePassphraseParams, RecipientInfo, RecipientKind, RemoveRecipientParams,
};

impl UnlockedVault {
    pub fn list_recipients(&self) -> Vec<RecipientInfo> {
        self.recipients()
    }

    /// Write vaultkey.age for the current recipients + keys, config.toml, and
    /// re-sign the manifest (it binds the config's security hash).
    fn persist_key_material(&mut self) -> Result<()> {
        self.write_vaultkey()?;
        self.write_config()?;
        self.save_manifest()
    }

    /// Encrypt the key document (current, previous and pending keys) to
    /// every recipient and write it atomically (fsync + rename).
    pub(crate) fn write_vaultkey(&mut self) -> Result<()> {
        let ct = encrypt_vaultkey(&self.config, &self.keys)?;
        write_atomic(&self.root.join(VAULTKEY_FILE), &ct)?;
        self.mark_verified(&[VAULTKEY_FILE]);
        Ok(())
    }

    /// Every session archive: committed (`sessions/`) and local (data dir).
    /// Unreadable directories are errors (never silently skipped).
    pub(crate) fn session_archives(&self) -> Result<Vec<PathBuf>> {
        let mut out = vec![];
        for dir in [
            self.root.join(SESSIONS_DIR),
            paths::sessions_dir(&self.config.vault.id),
        ] {
            let rd = match std::fs::read_dir(&dir) {
                Ok(rd) => rd,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => return Err(Error::io(format!("read {}", dir.display()), e)),
            };
            for f in rd {
                let f = f.map_err(|e| Error::io(format!("read {}", dir.display()), e))?;
                let p = f.path();
                if p.extension().is_some_and(|x| x == "age") {
                    out.push(p);
                }
            }
        }
        out.sort();
        Ok(out)
    }

    /// Every archive must be readable and decrypt with a vault key.
    fn validate_archives(&self, archives: &[PathBuf]) -> Result<()> {
        for p in archives {
            let ct = std::fs::read(p).map_err(|e| Error::io(format!("read {}", p.display()), e))?;
            crypto::decrypt_with(&self.keys.all_identities(), &ct).map_err(|_| {
                Error::integrity(format!(
                    "session archive {} cannot be decrypted with the vault key; rotation aborted \
                     (restore or delete the archive first)",
                    p.display()
                ))
            })?;
        }
        Ok(())
    }

    pub fn add_recipient(&mut self, params: &AddRecipientParams) -> Result<Vec<RecipientInfo>> {
        self.ensure_writable()?;
        let label = params.label.trim();
        if label.is_empty() || label.len() > 128 || label.chars().any(char::is_control) {
            return Err(Error::invalid(
                "recipient label must be 1-128 printable characters",
            ));
        }
        if self.config.recipients.iter().any(|r| r.label == label) {
            return Err(Error::AlreadyExists(format!("recipient `{label}`")));
        }
        let key = params.public_key.trim().to_owned();
        let parsed = crypto::parse_recipient(&key)?;
        if self.config.recipients.iter().any(|r| r.public_key == key) {
            return Err(Error::AlreadyExists(
                "a recipient with this public key".into(),
            ));
        }
        let backup = self.config.clone();
        self.config.recipients.push(RecipientDoc {
            label: label.to_owned(),
            kind: recipient_kind_str(parsed.kind).into(),
            public_key: key,
            added_at: Some(now()),
        });
        if let Err(e) = self.persist_key_material() {
            self.config = backup;
            return Err(e);
        }
        self.commit(&format!("recipients: add {label}"))?;
        Ok(self.recipients())
    }

    /// Remove a recipient. History stays readable to them; call
    /// [`UnlockedVault::rotate`] to protect future versions.
    pub fn remove_recipient(
        &mut self,
        params: &RemoveRecipientParams,
    ) -> Result<Vec<RecipientInfo>> {
        self.ensure_writable()?;
        let Some(r) = self
            .config
            .recipients
            .iter()
            .find(|r| r.label == params.label)
        else {
            return Err(Error::NotFound(format!("recipient `{}`", params.label)));
        };
        if parse_recipient_kind(&r.kind) == RecipientKind::Passphrase {
            return Err(Error::invalid(
                "the passphrase recipient cannot be removed; change the passphrase instead",
            ));
        }
        if self.config.recipients.len() <= 1 {
            return Err(Error::invalid("cannot remove the last recipient"));
        }
        let backup = self.config.clone();
        self.config.recipients.retain(|r| r.label != params.label);
        if let Err(e) = self.persist_key_material() {
            self.config = backup;
            // Persistence spans several atomic files. Re-read what reached
            // disk; incomplete state must block writes, not appear healthy.
            let _ = self.reload();
            return Err(e);
        }
        self.commit(&format!("recipients: remove {}", params.label))?;
        Ok(self.recipients())
    }

    /// New vault key + signing key; re-encrypt everything; one commit.
    /// Resumes an interrupted rotation instead of starting a new one.
    pub fn rotate(&mut self) -> Result<Vec<RecipientInfo>> {
        self.ensure_writable()?;
        // 1. Validate everything before touching disk.
        for p in self.manifest.profiles.clone() {
            self.read_profile_doc(&p.id)?;
            for e in &p.entries {
                self.read_entry_doc(&e.id)?;
            }
        }
        let archives = self.session_archives()?;
        self.validate_archives(&archives)?;

        // 2. Durably persist the next keys next to the current ones.
        if !self.rotation_pending() {
            let head = self.chain.head().clone();
            if head.signing_public_key
                != crypto::encode_verifying_key(&self.keys.signing.verifying_key())
            {
                return Err(Error::integrity(
                    "the keychain head is not the current signing key",
                ));
            }
            let vault_key = SecretKey32::generate();
            let identity = Box::new(crypto::vault_identity(&vault_key));
            let signing = Box::new(crypto::generate_signing_key());
            let record = trust::new_record(
                head.seq + 1,
                &self.config.vault.id,
                &signing.verifying_key(),
                &identity.to_public().to_string(),
                Some(&head),
                &self.keys.signing,
            );
            self.keys.next = Some(Box::new(PendingKeys {
                vault_key,
                identity,
                signing,
                record,
            }));
            if let Err(e) = self.write_vaultkey() {
                self.keys.next = None;
                return Err(e);
            }
            self.keys.on_disk_pending = true;
        }

        // 3. Re-encrypt, switch, re-sign, finalize.
        if let Err(e) = self.complete_rotation(&archives) {
            let _ = self.reload();
            return Err(e);
        }
        self.commit("vault: rotate vault key")?;
        Ok(self.recipients())
    }

    /// Finish a rotation whose next keys are persisted in `vaultkey.age`
    /// (also used by `doctor --fix`). Idempotent: every step checks what is
    /// already done. Does not commit.
    pub(crate) fn complete_rotation(&mut self, archives: &[PathBuf]) -> Result<()> {
        // a. Session archives → the new key (temp file + rename each).
        let target: age::x25519::Identity = match &self.keys.next {
            Some(n) => (*n.identity).clone(),
            None => (*self.keys.identity).clone(),
        };
        for p in archives {
            let ct = std::fs::read(p).map_err(|e| Error::io(format!("read {}", p.display()), e))?;
            if crypto::decrypt_with(&[&target as &dyn age::Identity], &ct).is_ok() {
                continue;
            }
            let pt: Zeroizing<Vec<u8>> = crypto::decrypt_with(&self.keys.all_identities(), &ct)
                .map_err(|_| {
                    Error::integrity(format!(
                        "session archive {} cannot be decrypted",
                        p.display()
                    ))
                })?;
            write_atomic(p, &crypto::encrypt_to_identity(&target, &pt)?)?;
        }
        // b. Put the new keys in force (in memory).
        let rec = self.keys.promote();
        let cur = crypto::encode_verifying_key(&self.keys.signing.verifying_key());
        // c. Keychain: append the record signed by the old key.
        if self.chain.head().signing_public_key != cur {
            let rec = rec.ok_or_else(|| {
                Error::integrity("the keychain lacks the record of the current signing key")
            })?;
            if self.chain.records.len() as u64 != rec.seq {
                return Err(Error::integrity(
                    "keychain sequence mismatch during rotation",
                ));
            }
            self.chain.records.push(rec);
            self.chain.write(&self.root)?;
            self.mark_verified(&[KEYCHAIN_FILE]);
        }
        // d. Config names the new signing key.
        if self.config.vault.signing_public_key != cur {
            self.config.vault.signing_public_key = cur;
            self.write_config()?;
        }
        // e. Re-encrypt profiles/entries still under an old key and re-sign
        //    the manifest with the new key.
        let r = self.rebuild_manifest_from_disk(true)?;
        if let Some(i) = r.issues.first() {
            return Err(Error::integrity(format!(
                "rotation could not re-encrypt {}: {}",
                i.path.clone().unwrap_or_default(),
                i.message
            )));
        }
        // f. Drop the pending block.
        self.keys.on_disk_pending = false;
        self.write_vaultkey()?;
        self.load_and_verify();
        self.check_ok()
    }

    /// Verify the current passphrase, then re-key the passphrase recipient
    /// with a new salt.
    pub fn change_passphrase(&mut self, params: &ChangePassphraseParams) -> Result<()> {
        self.ensure_writable()?;
        check_passphrase_strength(&params.new_passphrase)?;
        let salt = self.config.kdf.salt_bytes()?;
        let mut kdf = self.config.kdf.params();
        let cur = crypto::derive_passphrase_identity(
            &SecretString::from(params.current.clone()),
            &salt,
            &kdf,
        )?;
        let pass = self
            .config
            .passphrase_recipient()
            .ok_or_else(|| Error::invalid("vault has no passphrase recipient"))?;
        if pass.public_key != crypto::public_key(&cur) {
            return Err(Error::BadCredentials);
        }
        if crypto::validate_new_kdf(&kdf).is_err() {
            kdf = crypto::DEFAULT_KDF;
        }
        let new_salt = crypto::random_bytes::<{ crypto::SALT_LEN }>();
        let new_id = crypto::derive_passphrase_identity(
            &SecretString::from(params.new_passphrase.clone()),
            &new_salt,
            &kdf,
        )?;
        let backup = self.config.clone();
        self.config.kdf = KdfDoc::new(&kdf, &new_salt);
        let pk = crypto::public_key(&new_id);
        for r in &mut self.config.recipients {
            if parse_recipient_kind(&r.kind) == RecipientKind::Passphrase {
                r.public_key = pk.clone();
            }
        }
        if let Err(e) = self.persist_key_material() {
            self.config = backup;
            return Err(e);
        }
        self.commit("vault: change passphrase")?;
        if matches!(self.secret, UnlockSecret::Passphrase(_)) {
            self.secret = UnlockSecret::Passphrase(new_id);
        }
        Ok(())
    }

    /// Public age recipient (`age1…`) of the current vault identity. Data
    /// encrypted to it (e.g. browser session archives written after the
    /// vault locked) is readable with [`UnlockedVault::decrypt_blob`].
    pub fn vault_public_key(&self) -> String {
        self.keys.identity.to_public().to_string()
    }

    /// Encrypt arbitrary data with the vault key (session archives).
    pub fn encrypt_blob(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        self.encrypt(plaintext)
    }

    /// Decrypt data encrypted with the current or a previous vault key.
    pub fn decrypt_blob(&self, ciphertext: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
        self.decrypt(ciphertext, "archive")
    }
}

#[cfg(test)]
mod tests {
    use super::super::vault::test_vault;
    use crate::protocol::*;

    #[test]
    fn add_list_remove() {
        let t = tempfile::tempdir().unwrap();
        let mut v = test_vault(t.path());
        let pk = age::x25519::Identity::generate().to_public().to_string();
        let l = v
            .add_recipient(&AddRecipientParams {
                label: "bob".into(),
                public_key: pk,
            })
            .unwrap();
        assert_eq!(l.len(), 2);
        assert_eq!(v.list_recipients().len(), 2);
        assert!(
            v.add_recipient(&AddRecipientParams {
                label: " ".into(),
                public_key: "x".into()
            })
            .is_err()
        );
        let l = v
            .remove_recipient(&RemoveRecipientParams {
                label: "bob".into(),
            })
            .unwrap();
        assert_eq!(l.len(), 1);
        assert!(
            v.remove_recipient(&RemoveRecipientParams {
                label: "passphrase".into()
            })
            .is_err()
        );
        assert!(v.integrity_problems().is_empty());
    }

    #[test]
    fn rotate_and_blobs() {
        let t = tempfile::tempdir().unwrap();
        let mut v = test_vault(t.path());
        let ct = v.encrypt_blob(b"session").unwrap();
        v.rotate().unwrap();
        // Blobs encrypted with the previous key stay readable.
        assert_eq!(&v.decrypt_blob(&ct).unwrap()[..], b"session");
        let ct2 = v.encrypt_blob(b"new").unwrap();
        assert_ne!(ct, ct2);
        assert_eq!(&v.decrypt_blob(&ct2).unwrap()[..], b"new");
    }

    #[test]
    fn failed_removal_preserves_live_recipients_and_reopens() {
        let t = tempfile::tempdir().unwrap();
        let mut v = test_vault(t.path());
        v.add_recipient(&AddRecipientParams {
            label: "recovery-device".into(),
            public_key: age::x25519::Identity::generate().to_public().to_string(),
        })
        .unwrap();
        let before = v.list_recipients();
        let path = v.path().join(super::VAULTKEY_FILE);
        let original = std::fs::read(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();
        // Inject a rename failure at persistence, rather than exercising the
        // pre-write tamper check. A directory fails even when run as root.
        v.mark_verified(&[super::VAULTKEY_FILE]);
        let error = v
            .remove_recipient(&RemoveRecipientParams {
                label: "recovery-device".into(),
            })
            .unwrap_err();
        assert_eq!(error.code(), ErrorCode::Io);
        assert_eq!(v.list_recipients(), before);

        std::fs::remove_dir(&path).unwrap();
        std::fs::write(&path, original).unwrap();
        let reopened = super::super::vault::Vault::open(v.path())
            .unwrap()
            .unlock_passphrase("correct horse battery staple")
            .unwrap();
        assert!(reopened.integrity_problems().is_empty());
        assert_eq!(reopened.list_recipients(), before);
        v.reload().unwrap();
        v.remove_recipient(&RemoveRecipientParams {
            label: "recovery-device".into(),
        })
        .unwrap();
        assert_eq!(v.list_recipients().len(), 1);
    }

    #[test]
    fn partially_persisted_removal_blocks_live_and_reopened_writes() {
        let t = tempfile::tempdir().unwrap();
        let mut v = test_vault(t.path());
        v.add_recipient(&AddRecipientParams {
            label: "device".into(),
            public_key: age::x25519::Identity::generate().to_public().to_string(),
        })
        .unwrap();
        let head = v.git().log(1).unwrap()[0].commit.clone();
        let path = v.path().join(super::MANIFEST_SIG_FILE);
        let original = std::fs::read(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();
        v.mark_verified(&[super::MANIFEST_SIG_FILE]);
        assert_eq!(
            v.remove_recipient(&RemoveRecipientParams {
                label: "device".into(),
            })
            .unwrap_err()
            .code(),
            ErrorCode::Io
        );
        assert!(!v.integrity_problems().is_empty());
        assert_eq!(v.rotate().unwrap_err().code(), ErrorCode::Integrity);
        assert_eq!(v.git().log(1).unwrap()[0].commit, head);

        // Removing the obstruction cannot authenticate the partially written
        // manifest: the previous signature must still fail closed on reopen.
        std::fs::remove_dir(&path).unwrap();
        std::fs::write(&path, original).unwrap();
        let mut reopened = super::super::vault::Vault::open(v.path())
            .unwrap()
            .unlock_passphrase("correct horse battery staple")
            .unwrap();
        assert!(!reopened.integrity_problems().is_empty());
        assert_eq!(reopened.rotate().unwrap_err().code(), ErrorCode::Integrity);
    }
}
