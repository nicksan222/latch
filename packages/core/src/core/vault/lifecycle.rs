//! Credentials and transitions between located, locked and unlocked vaults.

use age::x25519;
use secrecy::SecretString;
use std::path::{Path, PathBuf};
use zeroize::Zeroizing;

use super::integrity::clone_marker;
use super::{IntegrityProblem, UnlockedVault, Vault, empty_manifest, load_state};
use crate::core::crypto;
use crate::core::error::{Error, Result};
use crate::core::format::*;
use crate::core::git::Git;
use crate::core::trust::{self, TrustInfo, TrustState};
use crate::protocol::{InitParams, InitResult, UnlockParams};

/// How to unlock.
pub enum Credentials {
    Passphrase(SecretString),
    /// Path to an age identity file or unencrypted OpenSSH private key.
    IdentityFile(PathBuf),
}

impl Credentials {
    /// From wire params: exactly one of passphrase / identity_file.
    pub fn from_params(p: &UnlockParams) -> Result<Self> {
        match (&p.passphrase, &p.identity_file) {
            (Some(pw), None) => Ok(Credentials::Passphrase(SecretString::from(pw.clone()))),
            (None, Some(f)) => Ok(Credentials::IdentityFile(PathBuf::from(f))),
            _ => Err(Error::invalid(
                "provide exactly one of passphrase or identityFile",
            )),
        }
    }
}

/// Identity material retained while unlocked so `vaultkey.age` can be
/// re-read after a sync brings in a rotated key. Send + Sync.
pub(crate) enum UnlockSecret {
    Passphrase(x25519::Identity),
    IdentityFile {
        data: Zeroizing<Vec<u8>>,
        name: String,
    },
}

impl UnlockSecret {
    pub(crate) fn identities(&self) -> Result<Vec<Box<dyn age::Identity>>> {
        Ok(match self {
            UnlockSecret::Passphrase(id) => vec![Box::new(id.clone())],
            UnlockSecret::IdentityFile { data, name } => crypto::identities_from_bytes(data, name)?,
        })
    }
}

impl Vault {
    /// Open an existing vault directory (no git required for reading).
    pub fn open(path: impl AsRef<Path>) -> Result<Vault> {
        let root = crate::core::paths::expand_vault_home(path.as_ref())?;
        let root = root
            .canonicalize()
            .map_err(|_| Error::NoVault(root.display().to_string()))?;
        read_version(&root)?;
        let config = ConfigDoc::read(&root)?;
        Ok(Vault { root, config })
    }

    /// `git clone` then open (locked).
    pub fn clone_from(url: &str, path: impl AsRef<Path>) -> Result<Vault> {
        let path = crate::core::paths::expand_vault_home(path.as_ref())?;
        if path.exists()
            && std::fs::read_dir(&path)
                .map(|mut d| d.next().is_some())
                .unwrap_or(true)
        {
            return Err(Error::AlreadyExists(format!(
                "{} (not empty)",
                path.display()
            )));
        }
        crate::core::git::clone(url, &path)?;
        match Vault::open(&path) {
            Ok(v) => Ok(v),
            Err(e) => {
                if matches!(e, Error::NoVault(_)) {
                    let _ = std::fs::remove_dir_all(&path);
                }
                Err(e)
            }
        }
    }

    /// Unlock with a passphrase or identity file.
    pub fn unlock(&self, creds: Credentials) -> Result<UnlockedVault> {
        let secret = match creds {
            Credentials::Passphrase(pw) => {
                let salt = self.config.kdf.salt_bytes()?;
                UnlockSecret::Passphrase(crypto::derive_passphrase_identity(
                    &pw,
                    &salt,
                    &self.config.kdf.params(),
                )?)
            }
            Credentials::IdentityFile(p) => UnlockSecret::IdentityFile {
                data: Zeroizing::new(read_file(&p)?),
                name: p.display().to_string(),
            },
        };
        UnlockedVault::open(self.root.clone(), self.config.clone(), secret)
    }

    pub fn unlock_with(&self, params: &UnlockParams) -> Result<UnlockedVault> {
        self.unlock(Credentials::from_params(params)?)
    }

    pub fn unlock_passphrase(&self, passphrase: &str) -> Result<UnlockedVault> {
        self.unlock(Credentials::Passphrase(SecretString::from(
            passphrase.to_owned(),
        )))
    }

    /// Create a new vault: files, git repo, first commit.
    pub fn init(params: &InitParams) -> Result<(UnlockedVault, InitResult)> {
        if let Some(u) = params
            .remote_url
            .as_deref()
            .filter(|u| !u.trim().is_empty())
        {
            crate::core::git::check_remote_url(u)?;
        }
        super::init::init(params)
    }

    /// Trust status before unlocking. The fingerprint is read from the
    /// (not yet verified) keychain; `FirstUse` means no key history is
    /// pinned on this machine, so unlocking will trust it on first use.
    pub fn trust_info(&self) -> Result<TrustInfo> {
        let chain = KeychainDoc::read(&self.root)?;
        let pinned = trust::pinned(&self.config.vault.id)?;
        let state = if pinned.is_empty() {
            TrustState::FirstUse
        } else if trust::relation(&pinned, &chain.records)? == trust::Relation::Stale {
            TrustState::Stale
        } else {
            TrustState::Pinned
        };
        Ok(TrustInfo {
            state,
            fingerprint: trust::fingerprint(&chain.records[0]),
            key_seq: chain.head().seq,
        })
    }

    /// Forget the pinned key history of this vault (explicit user action
    /// after verifying a new fingerprint out of band); the next unlock trusts
    /// the repository's keychain on first use. Also clears this clone's
    /// vault-id marker. Returns whether a pin existed.
    pub fn forget_trust(&self) -> Result<bool> {
        if let Some(m) = clone_marker(&Git::new(&self.root)) {
            let _ = std::fs::remove_file(m);
        }
        trust::forget(&self.config.vault.id)
    }
}

impl UnlockedVault {
    pub(super) fn open(
        root: PathBuf,
        _config: ConfigDoc,
        secret: UnlockSecret,
    ) -> Result<UnlockedVault> {
        let st = load_state(&root, &secret)?;
        let consent = trust::consent(&st.config.vault.id, &root);
        let mut v = UnlockedVault {
            git: Git::new(&root),
            root,
            manifest: empty_manifest(&st.config),
            config: st.config,
            keys: st.keys,
            secret,
            manifest_ct_hash: String::new(),
            problems: vec![],
            chain: st.chain,
            trust_state: if st.first_use {
                TrustState::FirstUse
            } else if st.relation == trust::Relation::Stale {
                TrustState::Stale
            } else {
                TrustState::Pinned
            },
            verified: vec![],
            verified_onto: None,
            consent,
            remembered_config: String::new(),
        };
        v.load_and_verify();
        Ok(v)
    }

    /// Re-read config, keychain, keys and manifest from disk and re-verify
    /// everything (including the trust anchor). On failure the handle keeps
    /// its previous (trusted) keys and is blocked.
    pub fn reload(&mut self) -> Result<()> {
        match load_state(&self.root, &self.secret) {
            Ok(st) => {
                self.config = st.config;
                self.chain = st.chain;
                self.keys = st.keys;
                if self.trust_state != TrustState::FirstUse {
                    self.trust_state = if st.relation == trust::Relation::Stale {
                        TrustState::Stale
                    } else {
                        TrustState::Pinned
                    };
                }
                self.load_and_verify();
                Ok(())
            }
            Err(e) => {
                self.problems = vec![IntegrityProblem {
                    code: "untrusted_state",
                    message: e.to_string(),
                    path: None,
                }];
                Err(e)
            }
        }
    }

    /// Zeroize keys and return the locked handle.
    pub fn lock(self) -> Vault {
        Vault {
            root: self.root.clone(),
            config: self.config.clone(),
        }
        // `self` drops here: SecretKey32/SigningKey/identities zeroize.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::vault::uninitialized_status;
    #[test]
    fn credentials_params() {
        let p = UnlockParams {
            passphrase: Some("x".into()),
            identity_file: None,
            timeout_secs: None,
        };
        assert!(matches!(
            Credentials::from_params(&p),
            Ok(Credentials::Passphrase(_))
        ));
        let p = UnlockParams {
            passphrase: None,
            identity_file: None,
            timeout_secs: None,
        };
        assert!(Credentials::from_params(&p).is_err());
        let p = UnlockParams {
            passphrase: Some("x".into()),
            identity_file: Some("f".into()),
            timeout_secs: None,
        };
        assert!(Credentials::from_params(&p).is_err());
    }

    #[test]
    fn open_missing_is_novault() {
        let t = tempfile::tempdir().unwrap();
        assert!(matches!(Vault::open(t.path()), Err(Error::NoVault(_))));
        assert!(matches!(
            Vault::open(t.path().join("nope")),
            Err(Error::NoVault(_))
        ));
        let s = uninitialized_status(Some(t.path()));
        assert!(!s.initialized && s.locked);
    }
}
