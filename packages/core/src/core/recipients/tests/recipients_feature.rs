// Recovery removal protects future ciphertext while preserving authorized history.

#[path = "../../../../tests/core_support/mod.rs"]
mod core_support;

use core_support::*;
use latch::core::{Credentials, Error, Vault, crypto, format};
use latch::protocol::*;
use secrecy::ExposeSecret;

#[test]
fn removed_recovery_identity_cannot_unlock_rotated_vault_but_history_survives() {
    setup();
    let t = tempfile::tempdir().unwrap();
    let path = t.path().join("vault");
    let mut params = init_params(&path);
    params.recovery_kit = true;
    let (mut vault, initialized) = Vault::init(&params).unwrap();
    let kit = initialized.recovery_kit.unwrap();
    // Recovery plaintext belongs outside the vault.
    let identity_file = t.path().join("recovery.txt");
    std::fs::write(&identity_file, &kit.recovery_secret_key).unwrap();
    let device = age::x25519::Identity::generate();
    let device_file = t.path().join("authorized.txt");
    std::fs::write(&device_file, device.to_string().expose_secret()).unwrap();
    vault
        .add_recipient(&AddRecipientParams {
            label: "authorized".into(),
            public_key: device.to_public().to_string(),
        })
        .unwrap();
    let profile = vault.create_profile(&profile_input("project")).unwrap();
    let entry = vault
        .create_entry(&entry_input(
            &profile.id,
            "token",
            EntryPayload::Secret(SecretData {
                value: Some("before rotation".into()),
            }),
        ))
        .unwrap();
    let original_commit = vault
        .history(&entry.id)
        .unwrap()
        .last()
        .unwrap()
        .commit
        .clone();
    let old_ciphertext = vault.encrypt_blob(b"historic session").unwrap();
    let old_key_document = std::fs::read(path.join(format::VAULTKEY_FILE)).unwrap();
    let recovery = Vault::open(&path)
        .unwrap()
        .unlock(Credentials::IdentityFile(identity_file.clone()))
        .unwrap();
    assert!(recovery.integrity_problems().is_empty());
    assert_eq!(
        &recovery.decrypt_blob(&old_ciphertext).unwrap()[..],
        b"historic session"
    );
    drop(recovery);

    vault
        .remove_recipient(&RemoveRecipientParams {
            label: "recovery".into(),
        })
        .unwrap();
    assert!(matches!(
        Vault::open(&path)
            .unwrap()
            .unlock(Credentials::IdentityFile(identity_file.clone())),
        Err(Error::BadCredentials)
    ));
    vault.rotate().unwrap();
    drop(vault);
    assert!(matches!(
        Vault::open(&path)
            .unwrap()
            .unlock(Credentials::IdentityFile(identity_file.clone())),
        Err(Error::BadCredentials)
    ));

    let mut authorized = Vault::open(&path)
        .unwrap()
        .unlock(Credentials::IdentityFile(device_file))
        .unwrap();
    assert!(authorized.integrity_problems().is_empty());
    assert_eq!(
        &authorized.decrypt_blob(&old_ciphertext).unwrap()[..],
        b"historic session"
    );
    let new_ciphertext = authorized.encrypt_blob(b"future session").unwrap();
    // A retained historical key document still decrypts old content, never
    // content encrypted after rotation.
    let identities = crypto::load_identity_file(&identity_file).unwrap();
    let refs: Vec<&dyn age::Identity> = identities.iter().map(|id| id.as_ref()).collect();
    let old_document =
        format::VaultKeyDoc::from_json(&crypto::decrypt_with(&refs, &old_key_document).unwrap())
            .unwrap();
    let old_key = crypto::SecretKey32::new(
        crypto::b64_decode(&old_document.vault_key)
            .unwrap()
            .try_into()
            .unwrap(),
    );
    let old_identity = crypto::vault_identity(&old_key);
    assert!(crypto::decrypt_with(&[&old_identity], &new_ciphertext).is_err());
    assert!(crypto::decrypt_with(&[&old_identity], &old_ciphertext).is_ok());
    authorized
        .update_entry(&EntryUpdateParams {
            id: entry.id.clone(),
            input: entry_input(
                &profile.id,
                "token",
                EntryPayload::Secret(SecretData {
                    value: Some("after rotation".into()),
                }),
            ),
        })
        .unwrap();
    authorized
        .restore(&RestoreParams {
            id: entry.id.clone(),
            commit: original_commit,
        })
        .unwrap();
    assert_eq!(
        authorized
            .reveal(&RevealParams {
                id: entry.id,
                field: "value".into()
            })
            .unwrap()
            .value,
        "before rotation"
    );
    assert!(unlock(&path).integrity_problems().is_empty());
}
