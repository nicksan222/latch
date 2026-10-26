// Recipients add/remove, rotation, passphrase change.

#[path = "../../../../tests/core_support/mod.rs"]
mod core_support;

use core_support::*;
use latch::core::{Error, Vault, format};
use latch::protocol::*;
use secrecy::ExposeSecret;

fn identity_file(dir: &std::path::Path) -> (std::path::PathBuf, String) {
    let id = age::x25519::Identity::generate();
    let p = dir.join(format!("id-{}.txt", latch::core::ids::new_id()));
    std::fs::write(&p, format!("{}\n", id.to_string().expose_secret())).unwrap();
    (p, id.to_public().to_string())
}

fn unlock_file(
    path: &std::path::Path,
    idf: &std::path::Path,
) -> latch::core::Result<latch::core::UnlockedVault> {
    Vault::open(path).unwrap().unlock_with(&UnlockParams {
        passphrase: None,
        identity_file: Some(idf.display().to_string()),
        timeout_secs: None,
    })
}

#[test]
fn add_remove_rotate() {
    let t = tempfile::tempdir().unwrap();
    let path = t.path().join("v");
    let mut v = new_vault(&path);
    let p = v.create_profile(&profile_input("acme")).unwrap();
    let e = v
        .create_entry(&entry_input(
            &p.id,
            "s",
            EntryPayload::Secret(SecretData {
                value: Some("v1".into()),
            }),
        ))
        .unwrap();
    let (bob_file, bob_pk) = identity_file(t.path());

    assert!(matches!(
        unlock_file(&path, &bob_file),
        Err(Error::BadCredentials)
    ));
    let list = v
        .add_recipient(&AddRecipientParams {
            label: "bob@laptop".into(),
            public_key: bob_pk.clone(),
        })
        .unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[1].kind, RecipientKind::Age);
    assert!(list[1].added_at.is_some());
    assert_eq!(
        v.git().log(1).unwrap()[0].message,
        "recipients: add bob@laptop"
    );
    // Duplicates rejected.
    assert_eq!(
        v.add_recipient(&AddRecipientParams {
            label: "bob@laptop".into(),
            public_key: bob_pk.clone()
        })
        .unwrap_err()
        .code(),
        ErrorCode::AlreadyExists
    );
    assert_eq!(
        v.add_recipient(&AddRecipientParams {
            label: "other".into(),
            public_key: bob_pk.clone()
        })
        .unwrap_err()
        .code(),
        ErrorCode::AlreadyExists
    );
    assert_eq!(
        v.add_recipient(&AddRecipientParams {
            label: "bad".into(),
            public_key: "nonsense".into()
        })
        .unwrap_err()
        .code(),
        ErrorCode::InvalidParams
    );
    // SSH recipients are accepted.
    let ssh = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIHsKLqeplhpW+uObz5dvMgjz1OxfM/XXUB+VHtZ6isGN ci";
    let list = v
        .add_recipient(&AddRecipientParams {
            label: "ci".into(),
            public_key: ssh.into(),
        })
        .unwrap();
    assert_eq!(list[2].kind, RecipientKind::Ssh);

    // Bob can unlock with his identity file and read entries.
    let bob = unlock_file(&path, &bob_file).unwrap();
    assert!(bob.integrity_problems().is_empty());
    assert_eq!(
        bob.reveal(&RevealParams {
            id: e.id.clone(),
            field: "value".into()
        })
        .unwrap()
        .value,
        "v1"
    );
    let old_vaultkey_bob_can_read = std::fs::read(path.join(format::VAULTKEY_FILE)).unwrap();
    drop(bob);

    // Passphrase recipient and unknown labels cannot be removed.
    assert_eq!(
        v.remove_recipient(&RemoveRecipientParams {
            label: "passphrase".into()
        })
        .unwrap_err()
        .code(),
        ErrorCode::InvalidParams
    );
    assert_eq!(
        v.remove_recipient(&RemoveRecipientParams {
            label: "nobody".into()
        })
        .unwrap_err()
        .code(),
        ErrorCode::NotFound
    );

    let list = v
        .remove_recipient(&RemoveRecipientParams {
            label: "bob@laptop".into(),
        })
        .unwrap();
    assert_eq!(list.len(), 2);
    assert!(matches!(
        unlock_file(&path, &bob_file),
        Err(Error::BadCredentials)
    ));

    // Bob kept an old vaultkey.age: without rotation he could still decrypt
    // new entries with the unchanged vault key. Rotate fixes that.
    let entry_before = std::fs::read(path.join(format::entry_file(&p.id, &e.id))).unwrap();
    let list = v.rotate().unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(
        v.git().log(1).unwrap()[0].message,
        "vault: rotate vault key"
    );
    let entry_after = std::fs::read(path.join(format::entry_file(&p.id, &e.id))).unwrap();
    assert_ne!(entry_before, entry_after, "entries re-encrypted");
    let mut u = unlock(&path);
    assert!(
        u.integrity_problems().is_empty(),
        "{:?}",
        u.integrity_problems()
    );
    assert_eq!(
        u.reveal(&RevealParams {
            id: e.id.clone(),
            field: "value".into()
        })
        .unwrap()
        .value,
        "v1"
    );
    // Bob's old copy of vaultkey.age opens the old key, which no longer
    // decrypts the current entry ciphertext.
    let bob_ids = latch::core::crypto::load_identity_file(&bob_file).unwrap();
    let refs: Vec<&dyn age::Identity> = bob_ids.iter().map(|b| b.as_ref()).collect();
    let old_doc = latch::core::crypto::decrypt_with(&refs, &old_vaultkey_bob_can_read).unwrap();
    let old_doc = format::VaultKeyDoc::from_json(&old_doc).unwrap();
    let old_key = latch::core::crypto::SecretKey32::new(
        latch::core::crypto::b64_decode(&old_doc.vault_key)
            .unwrap()
            .try_into()
            .unwrap(),
    );
    let old_id = latch::core::crypto::vault_identity(&old_key);
    assert!(
        latch::core::crypto::decrypt_with(&[&old_id as &dyn age::Identity], &entry_after).is_err()
    );
    assert!(
        latch::core::crypto::decrypt_with(&[&old_id as &dyn age::Identity], &entry_before).is_ok()
    );

    // History from before the rotation stays readable (previous keys kept).
    let mut input = entry_input(
        &p.id,
        "s",
        EntryPayload::Secret(SecretData {
            value: Some("v2".into()),
        }),
    );
    input.notes = "after rotate".into();
    u.update_entry(&EntryUpdateParams {
        id: e.id.clone(),
        input,
    })
    .unwrap();
    let h = u.history(&e.id).unwrap();
    assert_eq!(h.len(), 3);
    assert!(h[0].changed_fields.contains(&"value".to_owned()));
    let first = h.last().unwrap().commit.clone();
    let restored = u
        .restore(&RestoreParams {
            id: e.id.clone(),
            commit: first,
        })
        .unwrap();
    assert_eq!(restored.notes, "some notes");
    assert_eq!(
        u.reveal(&RevealParams {
            id: e.id.clone(),
            field: "value".into()
        })
        .unwrap()
        .value,
        "v1"
    );
    let r = u
        .doctor(&DoctorParams {
            fix: false,
            rebuild_manifest: false,
            staged: false,
        })
        .unwrap();
    assert!(r.ok, "{:?}", r.issues);
}

#[test]
fn change_passphrase() {
    let t = tempfile::tempdir().unwrap();
    let path = t.path().join("v");
    let mut v = new_vault(&path);
    let p = v.create_profile(&profile_input("acme")).unwrap();
    let salt_before = v.config().kdf.salt.clone();
    let e = v
        .change_passphrase(&ChangePassphraseParams {
            current: "not the passphrase".into(),
            new_passphrase: "new passphrase 123".into(),
        })
        .unwrap_err();
    assert_eq!(e.code(), ErrorCode::BadCredentials);
    assert_eq!(
        v.change_passphrase(&ChangePassphraseParams {
            current: PASS.into(),
            new_passphrase: "short".into()
        })
        .unwrap_err()
        .code(),
        ErrorCode::InvalidParams
    );
    v.change_passphrase(&ChangePassphraseParams {
        current: PASS.into(),
        new_passphrase: "new passphrase 123".into(),
    })
    .unwrap();
    assert_ne!(v.config().kdf.salt, salt_before);
    let vault = Vault::open(&path).unwrap();
    assert!(matches!(
        vault.unlock_passphrase(PASS),
        Err(Error::BadCredentials)
    ));
    let u = vault.unlock_passphrase("new passphrase 123").unwrap();
    assert!(u.integrity_problems().is_empty());
    assert_eq!(u.get_profile(&p.id).unwrap().name, "acme");
    // The handle keeps working (and can still write).
    v.create_profile(&profile_input("after")).unwrap();
}
