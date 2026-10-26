use super::*;
use latch_rpc::PROTOCOL_VERSION;
use latch_rpc::v1;
use std::time::Duration;

#[tokio::test]
async fn maintenance_procedures_work_from_a_client_while_vault_is_locked() {
    let mut host = Harness::start().await;
    let hello = host
        .client
        .hello(request(v1::HelloParams {
            client: v1::ClientKind::Cli as i32,
            version: "external-test/1".into(),
        }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(hello.protocol, PROTOCOL_VERSION);
    assert!(!hello.agent_version.is_empty());
    let benchmark = host
        .client
        .vault_benchmark_kdf(request(v1::KdfBenchmarkParams { target_ms: 1 }))
        .await
        .unwrap()
        .into_inner();
    assert!(benchmark.params.unwrap().memory_kib >= 64 * 1024);
    host.init().await;
    host.client.vault_lock(request(v1::Empty {})).await.unwrap();
    let vault = host.path("vault");
    let located = host
        .client
        .vault_locate(request(v1::OpenParams {
            path: vault.display().to_string(),
        }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(std::path::Path::new(&located.text), vault);
    let report = host
        .client
        .doctor_scan(request(v1::OpenParams {
            path: located.text.clone(),
        }))
        .await
        .unwrap()
        .into_inner();
    assert!(report.ok && report.issues.is_empty());
    let input = v1::HookParams {
        path: located.text,
        executable: Some(host.path("bin/xterm").display().to_string()),
    };
    let installed = host
        .client
        .hooks_install(request(input.clone()))
        .await
        .unwrap()
        .into_inner();
    let installed_path = installed.path.expect("installed hook path");
    assert!(std::path::Path::new(&installed_path).is_file());
    assert!(
        std::fs::read_to_string(&installed_path)
            .unwrap()
            .contains("doctor --staged")
    );
    let removed = host
        .client
        .hooks_uninstall(request(input.clone()))
        .await
        .unwrap()
        .into_inner();
    assert!(removed.removed);
    assert!(!std::path::Path::new(&installed_path).exists());
    assert!(
        !host
            .client
            .hooks_uninstall(request(input))
            .await
            .unwrap()
            .into_inner()
            .removed
    );
    let outside = host.path("not-a-vault");
    std::fs::create_dir(&outside).unwrap();
    assert!(
        host.client
            .doctor_scan(request(v1::OpenParams {
                path: outside.display().to_string()
            }))
            .await
            .is_err()
    );
    assert_eq!(std::fs::read_dir(outside).unwrap().count(), 0);
}

#[tokio::test]
async fn modified_ciphertext_fails_closed_for_reads_writes_and_restart() {
    let mut host = Harness::start().await;
    host.init().await;
    let profile = host.profile("integrity").await;
    let entry = host
        .client
        .entries_create(request(v1::EntryInput {
            profile_id: profile.id.clone(),
            name: "protected".into(),
            payload: Some(v1::EntryPayload {
                data: Some(v1::entry_payload::Data::Secret(v1::SecretData {
                    value: Some(SECRET.into()),
                })),
            }),
            ..Default::default()
        }))
        .await
        .unwrap()
        .into_inner();
    // Simulate a hostile repository writer; alter ciphertext, never create plaintext.
    let ciphertext = host.path(&format!(
        "vault/profiles/{}/entries/{}.age",
        profile.id, entry.id
    ));
    let mut bytes = std::fs::read(&ciphertext).unwrap();
    let last = bytes.last_mut().unwrap();
    *last ^= 1;
    std::fs::write(&ciphertext, bytes).unwrap();
    let error = host
        .client
        .entries_get(request(v1::GetEntryParams {
            id: entry.id.clone(),
            reveal: true,
        }))
        .await
        .unwrap_err();
    expect_error(error, v1::ErrorCode::Integrity);
    expect_error(
        host.client
            .profiles_create(request(profile_input("must-not-persist")))
            .await
            .unwrap_err(),
        v1::ErrorCode::Integrity,
    );
    host.restart().await;
    let status = host
        .client
        .vault_unlock(request(v1::UnlockParams {
            passphrase: Some(PASS.into()),
            ..Default::default()
        }))
        .await
        .unwrap()
        .into_inner();
    // Unlock permits a repair-capable session; data access must remain blocked.
    assert!(!status.locked && !status.integrity_problems.is_empty());
    expect_error(
        host.client
            .entries_get(request(v1::GetEntryParams {
                id: entry.id.clone(),
                reveal: true,
            }))
            .await
            .unwrap_err(),
        v1::ErrorCode::Integrity,
    );
    expect_error(
        host.client
            .profiles_create(request(profile_input("still-blocked")))
            .await
            .unwrap_err(),
        v1::ErrorCode::Integrity,
    );
    let repair = host
        .client
        .doctor_run(request(v1::DoctorParams {
            fix: true,
            rebuild_manifest: false,
            staged: false,
        }))
        .await
        .unwrap()
        .into_inner();
    assert!(
        repair.ok,
        "restore authenticated ciphertext from clean Git history"
    );
    let restored = host
        .client
        .entries_get(request(v1::GetEntryParams {
            id: entry.id,
            reveal: true,
        }))
        .await
        .unwrap()
        .into_inner();
    let v1::entry_payload::Data::Secret(data) = restored.payload.unwrap().data.unwrap() else {
        panic!("secret kind");
    };
    assert!(data.value.as_deref() == Some(SECRET));
}

#[tokio::test]
async fn passive_status_polling_does_not_prevent_automatic_lock() {
    let mut host = Harness::start().await;
    host.init().await;
    let mut settings = host
        .client
        .settings_get(request(v1::Empty {}))
        .await
        .unwrap()
        .into_inner();
    settings.auto_lock_secs = 1;
    host.client
        .settings_update(request(settings))
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let status = host
                .client
                .vault_status(request(v1::Empty {}))
                .await
                .unwrap()
                .into_inner();
            if status.locked {
                break;
            }
            tokio::time::sleep(Duration::from_millis(80)).await;
        }
    })
    .await
    .expect("auto-lock despite continuous passive polling");
    expect_error(
        host.client
            .profiles_list(request(v1::Empty {}))
            .await
            .unwrap_err(),
        v1::ErrorCode::Locked,
    );
}
