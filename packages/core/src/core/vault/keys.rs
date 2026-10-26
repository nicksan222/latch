//! Vault key material, authenticated rotation and recipient encryption.

use age::x25519;
use ed25519_dalek::SigningKey;
use zeroize::Zeroizing;

use super::UnlockSecret;
use crate::core::crypto::{self, SecretKey32};
use crate::core::error::{Error, Result};
use crate::core::format::*;
use crate::core::trust;

/// Keys of a rotation in progress (see FORMAT.md §4.2), authenticated by
/// `record` (signed by the current signing key).
pub(crate) struct PendingKeys {
    pub vault_key: SecretKey32,
    pub identity: Box<x25519::Identity>,
    pub signing: Box<SigningKey>,
    pub record: KeyRecord,
}

/// Decrypted key material. Boxed so moves never leave copies behind;
/// `SecretKey32`, `SigningKey` and age identities zeroize on drop.
pub(crate) struct Keys {
    pub vault_key: SecretKey32,
    pub identity: Box<x25519::Identity>,
    pub signing: Box<SigningKey>,
    /// Keys replaced by `rotate` (newest first) for reading history.
    pub previous: Vec<(SecretKey32, Box<x25519::Identity>)>,
    /// Rotation in progress whose keys are not in force yet.
    pub next: Option<Box<PendingKeys>>,
    /// `vaultkey.age` on disk still carries a `next` block.
    pub on_disk_pending: bool,
}

fn key32(s: &str) -> Result<SecretKey32> {
    let v = Zeroizing::new(
        crypto::b64_decode(s).ok_or_else(|| Error::integrity("malformed vault key"))?,
    );
    SecretKey32::from_slice(&v).ok_or_else(|| Error::integrity("malformed vault key"))
}

fn signing_from(s: &str) -> Result<Box<SigningKey>> {
    let seed = key32(s)?;
    Ok(Box::new(SigningKey::from_bytes(seed.expose())))
}

impl Keys {
    pub fn generate() -> Self {
        let vault_key = SecretKey32::generate();
        let identity = Box::new(crypto::vault_identity(&vault_key));
        Keys {
            vault_key,
            identity,
            signing: Box::new(crypto::generate_signing_key()),
            previous: vec![],
            next: None,
            on_disk_pending: false,
        }
    }

    pub fn from_doc(d: &VaultKeyDoc) -> Result<Self> {
        let vault_key = key32(&d.vault_key)?;
        let signing = signing_from(&d.signing_private_key)?;
        let mut previous = vec![];
        for p in &d.previous_vault_keys {
            let k = key32(p)?;
            let id = Box::new(crypto::vault_identity(&k));
            previous.push((k, id));
        }
        let next = match &d.next {
            Some(n) => {
                let k = key32(&n.vault_key)?;
                Some(Box::new(PendingKeys {
                    identity: Box::new(crypto::vault_identity(&k)),
                    vault_key: k,
                    signing: signing_from(&n.signing_private_key)?,
                    record: n.record.clone(),
                }))
            }
            None => None,
        };
        Ok(Keys {
            identity: Box::new(crypto::vault_identity(&vault_key)),
            vault_key,
            signing,
            previous,
            on_disk_pending: next.is_some(),
            next,
        })
    }

    pub fn to_doc(&self) -> VaultKeyDoc {
        VaultKeyDoc {
            version: FORMAT_VERSION,
            vault_key: crypto::b64(self.vault_key.expose()),
            signing_private_key: crypto::b64(&self.signing.to_bytes()),
            previous_vault_keys: self
                .previous
                .iter()
                .map(|(k, _)| crypto::b64(k.expose()))
                .collect(),
            next: self.next.as_ref().map(|n| PendingKeysDoc {
                vault_key: crypto::b64(n.vault_key.expose()),
                signing_private_key: crypto::b64(&n.signing.to_bytes()),
                record: n.record.clone(),
            }),
        }
    }

    /// Put the pending keys in force: the current key becomes
    /// `previous[0]`. Returns the record that introduced them.
    pub fn promote(&mut self) -> Option<KeyRecord> {
        let n = *self.next.take()?;
        let old_key = std::mem::replace(&mut self.vault_key, n.vault_key);
        let old_id = std::mem::replace(&mut self.identity, n.identity);
        self.signing = n.signing;
        self.previous.insert(0, (old_key, old_id));
        Some(n.record)
    }

    /// Current identity first, then pending and previous ones.
    pub fn all_identities(&self) -> Vec<&dyn age::Identity> {
        let mut v: Vec<&dyn age::Identity> = vec![&*self.identity];
        if let Some(n) = &self.next {
            v.push(&*n.identity);
        }
        v.extend(
            self.previous
                .iter()
                .map(|(_, i)| &**i as &dyn age::Identity),
        );
        v
    }

    pub fn recipient(&self) -> String {
        self.identity.to_public().to_string()
    }
}

/// Decide which keys are in force and check them against the (verified)
/// keychain and the config:
/// - a pending rotation must be authenticated by the current key; it is in
///   force once its record is in the keychain AND the config names its key;
/// - the key in force must be the config's `signing_public_key`, the
///   keychain head (or the head's predecessor while the head is the pending
///   record), and its vault identity must match the record's recipient hash.
pub(crate) fn resolve_keys(
    config: &ConfigDoc,
    chain: &KeychainDoc,
    doc: &VaultKeyDoc,
) -> Result<Keys> {
    let mut keys = Keys::from_doc(doc)?;
    let cur_pub = crypto::encode_verifying_key(&keys.signing.verifying_key());
    let cur_idx = chain
        .records
        .iter()
        .rposition(|r| r.signing_public_key == cur_pub);
    if let Some(n) = &keys.next {
        let Some(ci) = cur_idx else {
            return Err(Error::integrity(
                "vaultkey.age: the current key is not in the keychain",
            ));
        };
        trust::verify_link(Some(&chain.records[ci]), &n.record, &config.vault.id)?;
        if n.record.signing_public_key != crypto::encode_verifying_key(&n.signing.verifying_key())
            || n.record.vault_recipient_hash
                != trust::recipient_hash(&n.identity.to_public().to_string())
        {
            return Err(Error::integrity(
                "vaultkey.age: pending keys do not match their keychain record",
            ));
        }
    }
    let pending_in_chain = keys
        .next
        .as_ref()
        .is_some_and(|n| chain.records.get(n.record.seq as usize) == Some(&n.record));
    let pending_pub = keys
        .next
        .as_ref()
        .map(|n| n.record.signing_public_key.clone());
    if pending_in_chain && pending_pub.as_deref() == Some(config.vault.signing_public_key.as_str())
    {
        keys.promote();
    }
    let eff = crypto::encode_verifying_key(&keys.signing.verifying_key());
    if eff != config.vault.signing_public_key {
        return Err(Error::integrity(
            "vault signing key does not match config.toml",
        ));
    }
    let head = chain.head();
    let rec = if head.signing_public_key == eff {
        head
    } else if pending_in_chain && keys.next.is_some() && chain.records.len() >= 2 {
        // Keychain already lists the pending record; config not switched yet.
        &chain.records[chain.records.len() - 2]
    } else {
        return Err(Error::integrity(
            "the vault signing key is not the latest key of the keychain",
        ));
    };
    if rec.signing_public_key != eff
        || rec.vault_recipient_hash != trust::recipient_hash(&keys.recipient())
    {
        return Err(Error::integrity(
            "the vault key does not match the keychain (vaultkey.age replaced?)",
        ));
    }
    Ok(keys)
}

/// Decrypt `vaultkey.age` bytes with the unlock secret.
pub(crate) fn decrypt_vaultkey(ct: &[u8], secret: &UnlockSecret) -> Result<VaultKeyDoc> {
    let ids = secret.identities()?;
    let refs: Vec<&dyn age::Identity> = ids.iter().map(|b| b.as_ref()).collect();
    let pt = crypto::decrypt_with(&refs, ct).map_err(|e| match e {
        crypto::DecryptFailure::NoMatchingKey => Error::BadCredentials,
        crypto::DecryptFailure::Corrupt => Error::integrity("vaultkey.age is corrupt"),
    })?;
    VaultKeyDoc::from_json(&pt)
}

/// Encrypt the vault key document to every configured recipient.
pub(crate) fn encrypt_vaultkey(config: &ConfigDoc, keys: &Keys) -> Result<Vec<u8>> {
    let parsed: Vec<crypto::ParsedRecipient> = config
        .recipients
        .iter()
        .map(|r| crypto::parse_recipient(&r.public_key))
        .collect::<Result<_>>()?;
    if parsed.is_empty() {
        return Err(Error::invalid("a vault needs at least one recipient"));
    }
    let refs: Vec<&dyn age::Recipient> = parsed
        .iter()
        .map(|p| p.recipient.as_ref() as &dyn age::Recipient)
        .collect();
    crypto::encrypt_to(&refs, &keys.to_doc().to_json())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keys_doc_roundtrip() {
        let mut k = Keys::generate();
        let old = SecretKey32::generate();
        let oid = Box::new(crypto::vault_identity(&old));
        k.previous.push((old, oid));
        let d = k.to_doc();
        let k2 = Keys::from_doc(&VaultKeyDoc::from_json(&d.to_json()).unwrap()).unwrap();
        assert_eq!(k.vault_key.expose(), k2.vault_key.expose());
        assert_eq!(k.signing.to_bytes(), k2.signing.to_bytes());
        assert_eq!(k2.previous.len(), 1);
        assert_eq!(k2.all_identities().len(), 2);
    }
}
