//! Local trust anchor and the signed key-transition chain.
//!
//! The vault repository may live on a hostile remote: everything in it
//! (config, `vaultkey.age`, manifest, signatures) can be replaced by an
//! attacker who generates their own keys and encrypts them to the public
//! recipient keys. What an attacker cannot do is sign with a key they never
//! held. So each machine pins, OUTSIDE the repository, the key history it
//! has verified:
//!
//! - `<config dir>/trust/<vault-id>.json` (0600): the verified
//!   [`KeyRecord`] chain (genesis first) and per-clone local consent for
//!   disclosure-affecting settings.
//! - `.latch/keychain.json` (in the repo): the full chain. Genesis is
//!   self-signed; every later record (written by `rotate`) is signed by the
//!   previous signing key.
//!
//! On every load the on-disk chain must be consistent with the pinned one:
//! equal, a signed extension (pin is advanced), or a prefix (a stale clone
//! that has not synced a rotation yet). Anything else — a different genesis,
//! a fork — is an `Integrity` error. The first time a vault id is seen the
//! chain is trusted on first use (TOFU); [`fingerprint`] identifies the
//! genesis so users can compare it out of band (recovery kit, `latch status`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use ed25519_dalek::{SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};

use super::crypto;
use super::error::{Error, Result};
use super::format::{KeyRecord, KeychainDoc, now, write_atomic};
use super::ids;

/// Serializes read-modify-write cycles of trust files within this process.
static LOCK: Mutex<()> = Mutex::new(());

/// How this machine relates to the vault's key history.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustState {
    /// No pin existed: the key history was trusted on first use during this
    /// unlock. Show [`TrustInfo::fingerprint`] and ask the user to compare it
    /// with another device or the recovery kit.
    FirstUse,
    /// The key history matches (or signedly extends) the pinned one.
    Pinned,
    /// This clone's key history is an older prefix of the pinned one (a
    /// rotation made elsewhere has not been synced into this clone yet).
    Stale,
}

/// Trust status shown to the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustInfo {
    pub state: TrustState,
    /// Stable vault fingerprint (genesis key), e.g. `3F2A-…` (8 groups).
    pub fingerprint: String,
    /// Sequence number of the key epoch in use (0 until the first rotation).
    pub key_seq: u64,
}

/// Per-clone local acknowledgement of disclosure-affecting settings. The
/// effective value of each setting is `config value AND consent`, so a
/// remote (even a legitimate teammate) cannot make this machine publish
/// names or push without the local user enabling it here.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Consent {
    #[serde(default)]
    pub names_in_commits: bool,
    #[serde(default)]
    pub auto_push: bool,
    #[serde(default)]
    pub sessions_commit: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct TrustDoc {
    version: u32,
    vault_id: String,
    /// Verified chain, genesis first. Empty = nothing pinned yet.
    #[serde(default)]
    records: Vec<KeyRecord>,
    #[serde(default)]
    pinned_at: String,
    /// Canonical clone path → consent.
    #[serde(default)]
    consent: BTreeMap<String, Consent>,
}

/// Where the trust file for `vault_id` lives.
pub fn trust_path(vault_id: &str) -> Result<PathBuf> {
    if !ids::is_valid_id(vault_id) {
        return Err(Error::integrity("config.toml: malformed vault id"));
    }
    Ok(trust_dir().join(format!("{vault_id}.json")))
}

fn trust_dir() -> PathBuf {
    #[cfg(test)]
    if std::env::var_os(super::paths::ENV_CONFIG_DIR).is_none() {
        // Unit tests never touch the developer's real config dir.
        static DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
        return DIR
            .get_or_init(|| {
                let d = std::env::temp_dir().join(format!(
                    "latch-unit-trust-{}-{}",
                    std::process::id(),
                    ids::new_id()
                ));
                let _ = std::fs::create_dir_all(&d);
                d
            })
            .clone();
    }
    super::paths::config_dir().join("trust")
}

fn load_doc(vault_id: &str) -> Result<Option<TrustDoc>> {
    let p = trust_path(vault_id)?;
    match std::fs::read(&p) {
        Ok(b) => {
            let d: TrustDoc = serde_json::from_slice(&b).map_err(|_| {
                Error::integrity(format!("trust file {} is malformed", p.display()))
            })?;
            if d.vault_id != vault_id {
                return Err(Error::integrity(format!(
                    "trust file {} belongs to another vault",
                    p.display()
                )));
            }
            Ok(Some(d))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(Error::io(format!("read {}", p.display()), e)),
    }
}

fn store_doc(d: &TrustDoc) -> Result<()> {
    let p = trust_path(&d.vault_id)?;
    let dir = p.parent().expect("trust file has a parent");
    super::paths::ensure_private_dir(dir)
        .map_err(|e| Error::io(format!("create {}", dir.display()), e))?;
    let mut bytes = serde_json::to_vec_pretty(d).expect("serializes");
    bytes.push(b'\n');
    write_atomic(&p, &bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| Error::io(format!("chmod {}", p.display()), e))?;
    }
    Ok(())
}

fn update<T>(vault_id: &str, f: impl FnOnce(&mut TrustDoc) -> Result<T>) -> Result<T> {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut d = load_doc(vault_id)?.unwrap_or_else(|| TrustDoc {
        version: 1,
        vault_id: vault_id.to_owned(),
        ..Default::default()
    });
    let r = f(&mut d)?;
    store_doc(&d)?;
    Ok(r)
}

/// The pinned chain for `vault_id` (empty when nothing is pinned).
pub fn pinned(vault_id: &str) -> Result<Vec<KeyRecord>> {
    Ok(load_doc(vault_id)?.map(|d| d.records).unwrap_or_default())
}

/// Pin `records` (a verified chain): store it when nothing is pinned or it
/// extends the pin; keep the pin when `records` is a prefix; refuse forks.
pub fn pin(vault_id: &str, records: &[KeyRecord]) -> Result<()> {
    update(vault_id, |d| {
        if relation(&d.records, records)? == Relation::Extends {
            d.records = records.to_vec();
            d.pinned_at = now();
        }
        Ok(())
    })
}

/// Forget the pin for a vault (explicit user action, e.g. after verifying a
/// new fingerprint out of band). Consent entries are kept. Returns whether a
/// pin existed.
pub fn forget(vault_id: &str) -> Result<bool> {
    if load_doc(vault_id)?.is_none() {
        return Ok(false);
    }
    update(vault_id, |d| {
        let had = !d.records.is_empty();
        d.records.clear();
        d.pinned_at.clear();
        Ok(had)
    })
}

fn clone_key(root: &Path) -> String {
    root.canonicalize()
        .unwrap_or_else(|_| root.to_path_buf())
        .display()
        .to_string()
}

/// Local consent for the clone at `root` (all false when never given).
pub fn consent(vault_id: &str, root: &Path) -> Consent {
    load_doc(vault_id)
        .ok()
        .flatten()
        .and_then(|d| d.consent.get(&clone_key(root)).copied())
        .unwrap_or_default()
}

pub fn set_consent(vault_id: &str, root: &Path, c: Consent) -> Result<()> {
    let key = clone_key(root);
    update(vault_id, |d| {
        d.consent.insert(key, c);
        Ok(())
    })
}

// ---------------------------------------------------------------------------
// Chain verification
// ---------------------------------------------------------------------------

/// Hash binding a vault identity recipient (`age1…`) into a record.
pub fn recipient_hash(recipient: &str) -> String {
    let mut v = b"latch/v1/vault-recipient\n".to_vec();
    v.extend_from_slice(recipient.as_bytes());
    crypto::hash(&v)
}

/// Create a record for `new_key` signed by `signer` (genesis: signer =
/// new_key, `prev` = None).
pub fn new_record(
    seq: u64,
    vault_id: &str,
    new_key: &VerifyingKey,
    recipient: &str,
    prev: Option<&KeyRecord>,
    signer: &SigningKey,
) -> KeyRecord {
    let mut r = KeyRecord {
        seq,
        vault_id: vault_id.to_owned(),
        signing_public_key: crypto::encode_verifying_key(new_key),
        vault_recipient_hash: recipient_hash(recipient),
        created_at: now(),
        prev: prev.map(KeyRecord::hash),
        signature: String::new(),
    };
    r.signature = crypto::sign(signer, &r.signed_bytes());
    r
}

/// Verify that `rec` is a valid successor of `prev` (or a self-signed
/// genesis when `prev` is None) for `vault_id`.
pub fn verify_link(prev: Option<&KeyRecord>, rec: &KeyRecord, vault_id: &str) -> Result<()> {
    let bad = |m: &str| {
        Err(Error::integrity(format!(
            "keychain record {}: {m}",
            rec.seq
        )))
    };
    if rec.vault_id != vault_id {
        return bad("belongs to another vault");
    }
    let own = crypto::decode_verifying_key(&rec.signing_public_key)?;
    let signer = match prev {
        None => {
            if rec.seq != 0 || rec.prev.is_some() {
                return bad("genesis must have seq 0 and no predecessor");
            }
            own
        }
        Some(p) => {
            if rec.seq != p.seq + 1 || rec.prev.as_deref() != Some(p.hash().as_str()) {
                return bad("does not follow its predecessor");
            }
            crypto::decode_verifying_key(&p.signing_public_key)?
        }
    };
    if !crypto::verify(&signer, &rec.signed_bytes(), &rec.signature) {
        return bad("signature is invalid");
    }
    Ok(())
}

/// Verify a whole chain (genesis self-signed, every link signed by its
/// predecessor, one vault id).
pub fn verify_chain(k: &KeychainDoc, vault_id: &str) -> Result<()> {
    let mut prev: Option<&KeyRecord> = None;
    for r in &k.records {
        verify_link(prev, r, vault_id)?;
        prev = Some(r);
    }
    Ok(())
}

/// How a (verified) chain relates to a pinned one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Relation {
    /// Nothing pinned yet.
    Unpinned,
    Same,
    /// Longer than the pin, which it contains as a prefix.
    Extends,
    /// A strict prefix of the pin.
    Stale,
}

/// Compare a chain with the pin; forks/different genesis → `Integrity`.
pub fn relation(pinned: &[KeyRecord], chain: &[KeyRecord]) -> Result<Relation> {
    if pinned.is_empty() {
        return Ok(if chain.is_empty() {
            Relation::Same
        } else {
            Relation::Extends
        });
    }
    let n = pinned.len().min(chain.len());
    if pinned[..n] != chain[..n] {
        return Err(Error::integrity(format!(
            "the vault's key history does not match the one pinned on this machine \
             (fingerprint {}); the repository may have been tampered with. If you \
             verified the new fingerprint out of band, reset the pin explicitly",
            fingerprint(&pinned[0])
        )));
    }
    Ok(match chain.len().cmp(&pinned.len()) {
        std::cmp::Ordering::Equal => Relation::Same,
        std::cmp::Ordering::Greater => Relation::Extends,
        std::cmp::Ordering::Less => Relation::Stale,
    })
}

/// Human-comparable fingerprint of a vault: BLAKE3 over the vault id and
/// the genesis signing key, 128 bits as 8 groups of 4 hex digits. Stable
/// across rotations (the chain links every later key to the genesis).
pub fn fingerprint(genesis: &KeyRecord) -> String {
    let mut v = b"latch/v1/fingerprint\n".to_vec();
    v.extend_from_slice(genesis.vault_id.as_bytes());
    v.push(b'\n');
    v.extend_from_slice(genesis.signing_public_key.as_bytes());
    let h = blake3::hash(&v).to_hex().to_ascii_uppercase();
    h.as_bytes()[..32]
        .chunks(4)
        .map(|c| std::str::from_utf8(c).expect("hex"))
        .collect::<Vec<_>>()
        .join("-")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chain(id: &str, n: usize) -> (KeychainDoc, Vec<SigningKey>) {
        let mut keys = vec![crypto::generate_signing_key()];
        let g = new_record(0, id, &keys[0].verifying_key(), "age1g", None, &keys[0]);
        let mut recs = vec![g];
        for i in 1..n {
            let k = crypto::generate_signing_key();
            let r = new_record(
                i as u64,
                id,
                &k.verifying_key(),
                "age1x",
                recs.last(),
                &keys[i - 1],
            );
            recs.push(r);
            keys.push(k);
        }
        (
            KeychainDoc {
                version: 1,
                records: recs,
            },
            keys,
        )
    }

    #[test]
    fn chain_verifies_and_detects_forgery() {
        let id = ids::new_id();
        let (k, keys) = chain(&id, 3);
        verify_chain(&k, &id).unwrap();
        assert!(verify_chain(&k, &ids::new_id()).is_err());
        // A record signed by an unrelated key is rejected.
        let mut bad = k.clone();
        let evil = crypto::generate_signing_key();
        bad.records[2] = new_record(
            2,
            &id,
            &evil.verifying_key(),
            "age1e",
            Some(&k.records[1]),
            &evil,
        );
        assert!(verify_chain(&bad, &id).is_err());
        // Tampered field invalidates the signature.
        let mut bad = k.clone();
        bad.records[1].vault_recipient_hash = recipient_hash("age1evil");
        assert!(verify_chain(&bad, &id).is_err());
        // Proper extension by the head key verifies.
        let mut ext = k.clone();
        let nk = crypto::generate_signing_key();
        ext.records.push(new_record(
            3,
            &id,
            &nk.verifying_key(),
            "age1n",
            Some(&k.records[2]),
            &keys[2],
        ));
        verify_chain(&ext, &id).unwrap();
        assert_eq!(
            relation(&k.records, &ext.records).unwrap(),
            Relation::Extends
        );
        assert_eq!(relation(&ext.records, &k.records).unwrap(), Relation::Stale);
        assert_eq!(relation(&k.records, &k.records).unwrap(), Relation::Same);
        assert_eq!(relation(&[], &k.records).unwrap(), Relation::Extends);
        let (other, _) = chain(&id, 3);
        assert!(relation(&k.records, &other.records).is_err());
    }

    #[test]
    fn fingerprint_shape() {
        let id = ids::new_id();
        let (k, _) = chain(&id, 1);
        let f = fingerprint(&k.records[0]);
        assert_eq!(f.len(), 8 * 4 + 7);
        assert_eq!(f, fingerprint(&k.records[0]));
    }

    #[test]
    fn pin_consent_forget() {
        let id = ids::new_id();
        let (k, _) = chain(&id, 2);
        assert!(pinned(&id).unwrap().is_empty());
        pin(&id, &k.records[..1]).unwrap();
        pin(&id, &k.records).unwrap();
        assert_eq!(pinned(&id).unwrap().len(), 2);
        // A prefix does not shrink the pin; a fork is refused.
        pin(&id, &k.records[..1]).unwrap();
        assert_eq!(pinned(&id).unwrap().len(), 2);
        let (other, _) = chain(&id, 2);
        assert!(pin(&id, &other.records).is_err());
        let root = std::env::temp_dir();
        assert_eq!(consent(&id, &root), Consent::default());
        let c = Consent {
            names_in_commits: true,
            ..Default::default()
        };
        set_consent(&id, &root, c).unwrap();
        assert_eq!(consent(&id, &root), c);
        assert!(forget(&id).unwrap());
        assert!(pinned(&id).unwrap().is_empty());
        assert_eq!(consent(&id, &root), c);
        assert!(trust_path("../evil").is_err());
    }
}
