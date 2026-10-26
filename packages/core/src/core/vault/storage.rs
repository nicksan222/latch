//! Encrypted document reads and atomic writes behind the vault integrity guard.

use zeroize::Zeroizing;

use super::UnlockedVault;
use crate::core::crypto;
use crate::core::error::{Error, Result};
use crate::core::format::*;
use crate::core::{ids, secrets};

impl UnlockedVault {
    pub(crate) fn encrypt(&self, pt: &[u8]) -> Result<Vec<u8>> {
        crypto::encrypt_to_identity(&self.keys.identity, pt)
    }

    pub(crate) fn decrypt(&self, ct: &[u8], what: &str) -> Result<Zeroizing<Vec<u8>>> {
        crypto::decrypt_with(&self.keys.all_identities(), ct)
            .map_err(|_| Error::integrity(format!("{what} cannot be decrypted")))
    }

    /// Decrypt + parse an entry ciphertext (no manifest hash check).
    pub(crate) fn decode_entry(&self, ct: &[u8]) -> Result<EntryDoc> {
        let pt = self.decrypt(ct, "entry")?;
        EntryDoc::from_json(&pt)
    }

    pub(crate) fn decode_profile(&self, ct: &[u8]) -> Result<ProfileDoc> {
        let pt = self.decrypt(ct, "profile")?;
        ProfileDoc::from_json(&pt)
    }

    /// Read an entry, checking the manifest hash and the embedded ids.
    pub fn read_entry_doc(&self, id: &str) -> Result<EntryDoc> {
        self.check_ok()?;
        ids::check_id(id)?;
        let (p, e) = self
            .manifest
            .entry(id)
            .ok_or_else(|| Error::NotFound("entry".into()))?;
        let rel = entry_file(&p.id, &e.id);
        let ct = read_file(&self.root.join(&rel))?;
        if crypto::hash(&ct) != e.hash {
            return Err(Error::integrity(format!(
                "{rel} does not match the manifest"
            )));
        }
        let doc = self.decode_entry(&ct)?;
        if doc.id != e.id || doc.profile_id != p.id {
            return Err(Error::integrity(format!("{rel} holds a different entry")));
        }
        Ok(doc)
    }

    pub(crate) fn read_profile_doc(&self, id: &str) -> Result<ProfileDoc> {
        self.check_ok()?;
        ids::check_id(id)?;
        let p = self
            .manifest
            .profile(id)
            .ok_or_else(|| Error::NotFound("profile".into()))?;
        let rel = profile_file(id);
        let ct = read_file(&self.root.join(&rel))?;
        if crypto::hash(&ct) != p.hash {
            return Err(Error::integrity(format!(
                "{rel} does not match the manifest"
            )));
        }
        let doc = self.decode_profile(&ct)?;
        if doc.id != id {
            return Err(Error::integrity(format!("{rel} holds a different profile")));
        }
        Ok(doc)
    }

    /// Encrypt and write an entry; returns its ciphertext hash.
    pub(crate) fn write_entry_doc(&self, doc: &EntryDoc) -> Result<String> {
        let json = doc.to_json();
        secrets::check_size(&json)?;
        let ct = self.encrypt(&json)?;
        write_atomic(&self.root.join(entry_file(&doc.profile_id, &doc.id)), &ct)?;
        Ok(crypto::hash(&ct))
    }

    pub(crate) fn write_profile_doc(&self, doc: &ProfileDoc) -> Result<String> {
        let ct = self.encrypt(&doc.to_json())?;
        write_atomic(&self.root.join(profile_file(&doc.id)), &ct)?;
        Ok(crypto::hash(&ct))
    }

    /// Encrypt, sign and write the in-memory manifest.
    pub(crate) fn save_manifest(&mut self) -> Result<()> {
        self.manifest.version = FORMAT_VERSION;
        self.manifest.updated_at = now();
        self.manifest.config_hash = self.config.security_hash();
        let json = Zeroizing::new(serde_json::to_vec(&self.manifest).expect("serializes"));
        let ct = self.encrypt(&json)?;
        let sig = crypto::sign(&self.keys.signing, &ct);
        write_atomic(&self.root.join(MANIFEST_FILE), &ct)?;
        write_atomic(
            &self.root.join(MANIFEST_SIG_FILE),
            format!("{sig}\n").as_bytes(),
        )?;
        self.manifest_ct_hash = crypto::hash(&ct);
        self.mark_verified(&[MANIFEST_FILE, MANIFEST_SIG_FILE]);
        self.remember_config();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn ciphertext_roundtrip_rejects_corruption() {
        let dir = tempfile::tempdir().unwrap();
        let vault = crate::core::vault::test_vault(dir.path());
        let plaintext = b"private storage roundtrip";
        let mut ciphertext = vault.encrypt(plaintext).unwrap();
        assert!(!ciphertext.windows(plaintext.len()).any(|w| w == plaintext));
        assert_eq!(&*vault.decrypt(&ciphertext, "test").unwrap(), plaintext);
        let last = ciphertext.len() - 1;
        ciphertext[last] ^= 1;
        assert!(vault.decrypt(&ciphertext, "test").is_err());
    }
}
