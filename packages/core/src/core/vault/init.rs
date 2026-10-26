//! Create encrypted vault state, recovery material and the initial commit.

use age::x25519;
use secrecy::SecretString;
use std::path::Path;

use super::{Keys, UnlockSecret, UnlockedVault, empty_manifest, encrypt_vaultkey, snapshot};
use crate::core::crypto;
use crate::core::error::{Error, Result};
use crate::core::format::*;
use crate::core::git::Git;
use crate::core::ids;
use crate::core::trust::{self, Consent, TrustState};
use crate::protocol::{InitParams, InitResult, RecipientKind, RecoveryKit};

pub(crate) fn check_passphrase_strength(pw: &str) -> Result<()> {
    if pw.chars().count() < 8 {
        return Err(Error::invalid("passphrase must be at least 8 characters"));
    }
    Ok(())
}

fn recovery_document(
    vault_id: &str,
    fingerprint: &str,
    pass_pk: &str,
    rec_pk: &str,
    rec_sk: &str,
    created: &str,
) -> String {
    format!(
        "# Latch recovery kit\n\n\
Keep this document offline (printed or on encrypted storage). Anyone holding the\n\
recovery secret key below and a copy of the vault repository can read every\n\
secret in the vault.\n\n\
- Vault id: `{vault_id}`\n\
- Vault fingerprint: `{fingerprint}` (compare it with what Latch shows when a\n\
  new device first unlocks this vault; a different value means the\n\
  repository's keys were replaced)\n\
- Created: {created}\n\
- Passphrase identity public key: `{pass_pk}`\n\
- Recovery public key: `{rec_pk}`\n\n\
## Recovery secret key\n\n\
```\n{rec_sk}\n```\n\n\
## How to use\n\n\
1. Save the secret key line above into a file, e.g. `recovery.txt` (mode 600).\n\
2. Clone the vault repository.\n\
3. Run `latch unlock --identity-file recovery.txt`, then change the passphrase.\n\n\
If the recovery key is ever exposed, remove the `recovery` recipient and run\n\
`latch recipients rotate`.\n"
    )
}

pub(super) fn init(params: &InitParams) -> Result<(UnlockedVault, InitResult)> {
    let root = crate::core::paths::expand_vault_home(Path::new(&params.path))?;
    if root.join(VERSION_FILE).exists() {
        return Err(Error::AlreadyExists(format!("vault at {}", root.display())));
    }
    if root.exists()
        && std::fs::read_dir(&root)
            .map_err(|e| Error::io(format!("read {}", root.display()), e))?
            .flatten()
            .any(|e| e.file_name() != ".git")
    {
        return Err(Error::invalid(format!("{} is not empty", root.display())));
    }
    check_passphrase_strength(&params.passphrase)?;
    let kdf = match &params.kdf {
        Some(k) => k.clone(),
        None => crypto::calibrate_kdf(1000)?.params,
    };
    crypto::validate_new_kdf(&kdf)?;
    if !crate::core::git::available() {
        return Err(Error::Git("git executable not found".into()));
    }
    std::fs::create_dir_all(&root)
        .map_err(|e| Error::io(format!("create {}", root.display()), e))?;
    let root = root
        .canonicalize()
        .map_err(|e| Error::io(format!("resolve {}", root.display()), e))?;

    let salt = crypto::random_bytes::<{ crypto::SALT_LEN }>();
    let pw = SecretString::from(params.passphrase.clone());
    let pass_id = crypto::derive_passphrase_identity(&pw, &salt, &kdf)?;
    let pass_pk = crypto::public_key(&pass_id);
    let keys = Keys::generate();
    let created = now();
    let vault_id = ids::new_id();
    let genesis = trust::new_record(
        0,
        &vault_id,
        &keys.signing.verifying_key(),
        &keys.recipient(),
        None,
        &keys.signing,
    );
    let fingerprint = trust::fingerprint(&genesis);
    let chain = KeychainDoc {
        version: FORMAT_VERSION,
        records: vec![genesis],
    };

    let mut recipients = vec![RecipientDoc {
        label: "passphrase".into(),
        kind: recipient_kind_str(RecipientKind::Passphrase).into(),
        public_key: pass_pk.clone(),
        added_at: Some(created.clone()),
    }];
    let mut kit = None;
    if params.recovery_kit {
        let rid = x25519::Identity::generate();
        let rpk = crypto::public_key(&rid);
        let rsk = {
            use secrecy::ExposeSecret;
            rid.to_string().expose_secret().to_owned()
        };
        recipients.push(RecipientDoc {
            label: "recovery".into(),
            kind: recipient_kind_str(RecipientKind::Recovery).into(),
            public_key: rpk.clone(),
            added_at: Some(created.clone()),
        });
        kit = Some(RecoveryKit {
            vault_id: vault_id.clone(),
            passphrase_public_key: pass_pk.clone(),
            recovery_public_key: rpk.clone(),
            document: recovery_document(&vault_id, &fingerprint, &pass_pk, &rpk, &rsk, &created),
            recovery_secret_key: rsk,
        });
    }

    let config = ConfigDoc {
        version: FORMAT_VERSION,
        kdf: KdfDoc::new(&kdf, &salt),
        vault: VaultMetaDoc {
            id: vault_id.clone(),
            created_at: created,
            signing_public_key: crypto::encode_verifying_key(&keys.signing.verifying_key()),
        },
        recipients,
        sessions: SessionsDoc::default(),
        sync: SyncDoc::default(),
        hooks: HooksDoc::default(),
    };

    let git = Git::new(&root);
    if !git.is_repo() {
        git.init(params.git_name.as_deref(), params.git_email.as_deref())?;
    } else if let (Some(n), Some(e)) = (&params.git_name, &params.git_email) {
        git.set_identity(n, e)?;
    }

    write_atomic(
        &root.join(VERSION_FILE),
        format!("{FORMAT_VERSION}\n").as_bytes(),
    )?;
    config.write(&root)?;
    chain.write(&root)?;
    write_atomic(
        &root.join(VAULTKEY_FILE),
        &encrypt_vaultkey(&config, &keys)?,
    )?;
    write_atomic(&root.join(README_FILE), readme(&vault_id).as_bytes())?;
    write_atomic(&root.join(GITATTRIBUTES_FILE), GITATTRIBUTES.as_bytes())?;
    write_atomic(&root.join(GITIGNORE_FILE), GITIGNORE.as_bytes())?;

    let mut v = UnlockedVault {
        git: git.clone(),
        manifest: empty_manifest(&config),
        root: root.clone(),
        config,
        keys,
        secret: UnlockSecret::Passphrase(pass_id),
        manifest_ct_hash: String::new(),
        problems: vec![],
        chain,
        trust_state: TrustState::Pinned,
        verified: vec![],
        verified_onto: None,
        consent: Consent::default(),
        remembered_config: String::new(),
    };
    v.save_manifest()?;
    if let Some(url) = params
        .remote_url
        .as_deref()
        .filter(|u| !u.trim().is_empty())
    {
        git.set_remote(url)?;
    }
    if v.config.hooks.pre_commit_scan {
        // An existing foreign hook is left alone (doctor reports it).
        match crate::core::doctor::install_hook(&git, None) {
            Ok(_) | Err(Error::AlreadyExists(_)) => {}
            Err(e) => return Err(e),
        }
    }
    v.commit("vault: init")?;
    v.verified = snapshot(&v.root);
    v.commit_trust();
    let status = v.status();
    Ok((
        v,
        InitResult {
            status,
            recovery_kit: kit,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recovery_doc_mentions_key() {
        let d = recovery_document(
            "vid",
            "ABCD-EF01",
            "age1p",
            "age1r",
            "AGE-SECRET-KEY-1XYZ",
            "now",
        );
        assert!(d.contains("AGE-SECRET-KEY-1XYZ") && d.contains("vid") && d.contains("ABCD-EF01"));
    }
}
