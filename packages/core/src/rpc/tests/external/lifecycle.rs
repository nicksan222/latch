use super::*;
use latch_rpc::v1;
use prost::Message;

fn id(id: &str) -> v1::IdParams {
    v1::IdParams { id: id.into() }
}

#[tokio::test]
async fn switching_registered_vaults_keeps_profiles_isolated() {
    let mut h = Harness::start().await;
    let first_id = h
        .init()
        .await
        .status
        .expect("first vault status")
        .vault_id
        .expect("first vault id");
    let first_profile = h.profile("first vault profile").await;
    let second = h
        .client
        .vault_init(request(v1::InitParams {
            path: h.path("vault-two").display().to_string(),
            passphrase: PASS.into(),
            kdf: Some(v1::KdfParams {
                memory_kib: 256,
                iterations: 1,
                parallelism: 1,
            }),
            git_name: Some("External RPC tests".into()),
            git_email: Some("grpc@example.invalid".into()),
            ..Default::default()
        }))
        .await
        .unwrap()
        .into_inner();
    let second_id = second
        .status
        .expect("second vault status")
        .vault_id
        .expect("second vault id");
    assert_ne!(first_id, second_id);
    let second_profile = h.profile("second vault profile").await;
    let registered = h
        .client
        .vault_list(request(v1::Empty {}))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(registered.items.len(), 2);
    assert!(
        registered
            .items
            .iter()
            .any(|v| v.active && v.vault_id == second_id)
    );

    let selected = h
        .client
        .vault_open(request(v1::OpenParams {
            path: h.path("vault").display().to_string(),
        }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(selected.vault_id.as_deref(), Some(first_id.as_str()));
    assert!(selected.locked);
    h.client
        .vault_unlock(request(v1::UnlockParams {
            passphrase: Some(PASS.into()),
            ..Default::default()
        }))
        .await
        .unwrap();
    let profiles = h
        .client
        .profiles_list(request(v1::Empty {}))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(profiles.items.len(), 1);
    assert_eq!(profiles.items[0].id, first_profile.id);

    let selected = h
        .client
        .vault_open(request(v1::OpenParams {
            path: h.path("vault-two").display().to_string(),
        }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(selected.vault_id.as_deref(), Some(second_id.as_str()));
    assert!(selected.locked);
    h.client
        .vault_unlock(request(v1::UnlockParams {
            passphrase: Some(PASS.into()),
            ..Default::default()
        }))
        .await
        .unwrap();
    let profiles = h
        .client
        .profiles_list(request(v1::Empty {}))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(profiles.items.len(), 1);
    assert_eq!(profiles.items[0].id, second_profile.id);
}

#[tokio::test]
async fn lifecycle_recovery_and_restart() {
    let mut h = Harness::start().await;
    let initial = h
        .client
        .vault_status(request(v1::Empty {}))
        .await
        .unwrap()
        .into_inner();
    assert!(!initial.initialized && initial.locked);
    expect_error(
        h.client
            .profiles_list(request(v1::Empty {}))
            .await
            .unwrap_err(),
        v1::ErrorCode::NoVault,
    );

    let initialized = h.init().await;
    let status = initialized.status.expect("status after init");
    assert!(status.initialized && !status.locked);
    assert!(status.vault_id.is_some());
    let kit = initialized.recovery_kit.expect("requested recovery kit");
    assert_eq!(Some(kit.vault_id.as_str()), status.vault_id.as_deref());
    assert!(!kit.recovery_secret_key.is_empty() && !kit.document.is_empty());
    assert!(kit.document.contains(&kit.vault_id));

    let listed = h
        .client
        .vault_list(request(v1::Empty {}))
        .await
        .unwrap()
        .into_inner();
    assert!(
        listed
            .items
            .iter()
            .any(|item| item.active && item.vault_id == kit.vault_id)
    );
    let opened = h
        .client
        .vault_open(request(v1::OpenParams {
            path: h.path("vault").display().to_string(),
        }))
        .await
        .unwrap()
        .into_inner();
    assert!(opened.initialized && !opened.locked);

    let a = h.profile("work").await;
    let b = h.profile("personal").await;
    assert_ne!(a.id, b.id);
    let profiles = h
        .client
        .profiles_list(request(v1::Empty {}))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(profiles.items.len(), 2);
    let got = h
        .client
        .profiles_get(request(id(&a.id)))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(got.name, "work");
    expect_error(
        h.client
            .profiles_create(request(profile_input("work")))
            .await
            .unwrap_err(),
        v1::ErrorCode::AlreadyExists,
    );
    let mut renamed = profile_input("work renamed");
    renamed.pinned = true;
    let updated = h
        .client
        .profiles_update(request(v1::ProfileUpdateParams {
            id: a.id.clone(),
            input: Some(renamed),
        }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(updated.id, a.id);
    assert!(updated.pinned);
    assert_eq!(updated.name, "work renamed");

    let trust = h
        .client
        .vault_trust(request(v1::Empty {}))
        .await
        .unwrap()
        .into_inner();
    assert!(!trust.fingerprint.is_empty());
    assert_eq!(trust.state, v1::VaultTrustState::Pinned as i32);
    let forgotten = h
        .client
        .vault_forget_trust(request(v1::ForgetTrustParams { confirm: true }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(forgotten.state, v1::VaultTrustState::FirstUse as i32);

    assert!(
        h.client
            .vault_lock(request(v1::Empty {}))
            .await
            .unwrap()
            .into_inner()
            .locked
    );
    expect_error(
        h.client.profiles_get(request(id(&a.id))).await.unwrap_err(),
        v1::ErrorCode::Locked,
    );
    let wrong_passphrase = "fictional-wrong-passphrase";
    let rejected = h
        .client
        .vault_unlock(request(v1::UnlockParams {
            passphrase: Some(wrong_passphrase.into()),
            ..Default::default()
        }))
        .await
        .unwrap_err();
    assert!(!rejected.message().contains(wrong_passphrase));
    let details = v1::RpcError::decode(rejected.details()).expect("typed credential error");
    assert!(!details.message.contains(wrong_passphrase));
    expect_error(rejected, v1::ErrorCode::BadCredentials);
    assert!(
        h.client
            .vault_status(request(v1::Empty {}))
            .await
            .unwrap()
            .into_inner()
            .locked
    );
    assert!(
        !h.client
            .vault_unlock(request(v1::UnlockParams {
                passphrase: Some(PASS.into()),
                ..Default::default()
            }))
            .await
            .unwrap()
            .into_inner()
            .locked
    );

    // Recover through the exported identity and public RPC, never vault internals.
    h.client.vault_lock(request(v1::Empty {})).await.unwrap();
    let recovery_path = h.path("recovery-identity.txt");
    std::fs::write(&recovery_path, &kit.recovery_secret_key).unwrap();
    let recovered = h
        .client
        .vault_unlock(request(v1::UnlockParams {
            identity_file: Some(recovery_path.display().to_string()),
            ..Default::default()
        }))
        .await
        .unwrap()
        .into_inner();
    assert!(!recovered.locked);

    let next_pass = "fictional-next-passphrase";
    h.client
        .vault_change_passphrase(request(v1::ChangePassphraseParams {
            current: PASS.into(),
            new_passphrase: next_pass.into(),
        }))
        .await
        .unwrap();
    h.client.vault_lock(request(v1::Empty {})).await.unwrap();
    expect_error(
        h.client
            .vault_unlock(request(v1::UnlockParams {
                passphrase: Some(PASS.into()),
                ..Default::default()
            }))
            .await
            .unwrap_err(),
        v1::ErrorCode::BadCredentials,
    );
    assert!(
        !h.client
            .vault_unlock(request(v1::UnlockParams {
                passphrase: Some(next_pass.into()),
                ..Default::default()
            }))
            .await
            .unwrap()
            .into_inner()
            .locked
    );
    h.restart().await;
    assert!(
        h.client
            .vault_status(request(v1::Empty {}))
            .await
            .unwrap()
            .into_inner()
            .locked
    );
    assert!(
        !h.client
            .vault_unlock(request(v1::UnlockParams {
                passphrase: Some(next_pass.into()),
                ..Default::default()
            }))
            .await
            .unwrap()
            .into_inner()
            .locked
    );
    assert_eq!(
        h.client
            .profiles_get(request(id(&a.id)))
            .await
            .unwrap()
            .into_inner()
            .name,
        "work renamed"
    );
    h.client.profiles_delete(request(id(&b.id))).await.unwrap();
    assert_eq!(
        h.client
            .profiles_list(request(v1::Empty {}))
            .await
            .unwrap()
            .into_inner()
            .items
            .len(),
        1
    );
}

#[tokio::test]
async fn settings_doctor_and_invalid_profile_writes() {
    let mut h = Harness::start().await;
    h.init().await;
    for name in ["".to_owned(), "invalid/name".to_owned(), "x".repeat(129)] {
        expect_error(
            h.client
                .profiles_create(request(profile_input(&name)))
                .await
                .unwrap_err(),
            v1::ErrorCode::InvalidParams,
        );
    }
    assert!(
        h.client
            .profiles_list(request(v1::Empty {}))
            .await
            .unwrap()
            .into_inner()
            .items
            .is_empty()
    );

    let initial = h
        .client
        .settings_get(request(v1::Empty {}))
        .await
        .unwrap()
        .into_inner();
    let mut changed = initial.clone();
    changed.theme = v1::Theme::Dark as i32;
    changed.reveal_hide_secs = 19;
    let saved = h
        .client
        .settings_update(request(changed))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(saved.theme, v1::Theme::Dark as i32);
    assert_eq!(saved.reveal_hide_secs, 19);
    let report = h
        .client
        .doctor_run(request(v1::DoctorParams::default()))
        .await
        .unwrap()
        .into_inner();
    assert!(report.ok);
    h.restart().await;
    let persisted = h
        .client
        .settings_get(request(v1::Empty {}))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(persisted.theme, v1::Theme::Dark as i32);
    assert_eq!(persisted.reveal_hide_secs, 19);
}
