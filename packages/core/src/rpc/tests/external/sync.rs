use super::*;
use latch_rpc::v1;

fn secret(profile_id: &str, name: &str, value: &str) -> v1::EntryInput {
    v1::EntryInput {
        profile_id: profile_id.into(),
        name: name.into(),
        payload: Some(v1::EntryPayload {
            data: Some(v1::entry_payload::Data::Secret(v1::SecretData {
                value: Some(value.into()),
            })),
        }),
        ..Default::default()
    }
}

fn both() -> v1::SyncParams {
    v1::SyncParams {
        mode: v1::SyncMode::Both as i32,
    }
}

#[tokio::test]
async fn local_bare_remote_clone_divergence_and_conflict_resolution() {
    let mut a = Harness::start().await;
    a.init().await;
    let remote = a.path("remote.git");
    let mut git = std::process::Command::new("git");
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("GIT_") {
            git.env_remove(key);
        }
    }
    let output = git
        .env("GIT_CONFIG_GLOBAL", a.path("gitconfig"))
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .args(["init", "--bare", "--quiet", "-b", "main"])
        .arg(&remote)
        .output()
        .unwrap();
    assert!(output.status.success(), "bare git remote setup failed");

    let initially_local = a
        .client
        .sync_status(request(v1::Empty {}))
        .await
        .unwrap()
        .into_inner();
    assert!(!initially_local.has_remote);
    let set = a
        .client
        .sync_set_remote(request(v1::SetRemoteParams {
            url: remote.display().to_string(),
        }))
        .await
        .unwrap()
        .into_inner();
    assert!(set.has_remote);
    let p = a.profile("shared").await;
    let shared = a
        .client
        .entries_create(request(secret(&p.id, "shared", "initial synthetic value")))
        .await
        .unwrap()
        .into_inner();
    let pushed = a
        .client
        .sync_run(request(both()))
        .await
        .unwrap()
        .into_inner();
    assert!(pushed.pushed > 0 && pushed.conflicts.is_empty());
    assert_eq!(pushed.status.as_ref().unwrap().ahead, 0);

    let mut b = Harness::start().await;
    let clone = b
        .client
        .vault_clone(request(v1::CloneParams {
            url: remote.display().to_string(),
            path: b.path("vault").display().to_string(),
        }))
        .await
        .unwrap()
        .into_inner();
    assert!(clone.initialized && clone.locked && clone.has_remote);
    b.client
        .vault_unlock(request(v1::UnlockParams {
            passphrase: Some(PASS.into()),
            ..Default::default()
        }))
        .await
        .unwrap();
    let from_clone = b
        .client
        .entries_reveal(request(v1::RevealParams {
            id: shared.id.clone(),
            field: "value".into(),
        }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(from_clone.value, "initial synthetic value");

    // Independent edits should merge through the same external service path.
    let from_a = a
        .client
        .entries_create(request(secret(&p.id, "only-a", "A synthetic value")))
        .await
        .unwrap()
        .into_inner();
    let from_b = b
        .client
        .entries_create(request(secret(&p.id, "only-b", "B synthetic value")))
        .await
        .unwrap()
        .into_inner();
    assert!(
        a.client
            .sync_run(request(both()))
            .await
            .unwrap()
            .into_inner()
            .pushed
            > 0
    );
    let merged = b
        .client
        .sync_run(request(both()))
        .await
        .unwrap()
        .into_inner();
    assert!(merged.pulled > 0 && merged.pushed > 0 && merged.conflicts.is_empty());
    assert_eq!(merged.status.as_ref().unwrap().ahead, 0);
    a.client.sync_run(request(both())).await.unwrap();
    let list = a
        .client
        .entries_list(request(v1::ListEntriesParams {
            profile_id: Some(p.id.clone()),
            ..Default::default()
        }))
        .await
        .unwrap()
        .into_inner();
    assert!(list.items.iter().any(|e| e.id == from_a.id));
    assert!(list.items.iter().any(|e| e.id == from_b.id));

    // Two edits of one encrypted entry produce a typed conflict. The selected
    // local version is returned by reveal after resolution and sync.
    a.client
        .entries_update(request(v1::EntryUpdateParams {
            id: shared.id.clone(),
            input: Some(secret(&p.id, "shared", "A conflict value")),
        }))
        .await
        .unwrap();
    b.client
        .entries_update(request(v1::EntryUpdateParams {
            id: shared.id.clone(),
            input: Some(secret(&p.id, "shared", "B conflict value")),
        }))
        .await
        .unwrap();
    assert!(
        a.client
            .sync_run(request(both()))
            .await
            .unwrap()
            .into_inner()
            .pushed
            > 0
    );
    let conflicting = b
        .client
        .sync_run(request(both()))
        .await
        .unwrap()
        .into_inner();
    assert!(
        conflicting
            .conflicts
            .iter()
            .any(|c| c.entry_id == shared.id)
    );
    let detail = b
        .client
        .sync_conflict(request(v1::IdParams {
            id: shared.id.clone(),
        }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(detail.entry_id, shared.id);
    assert!(detail.mine.is_some() && detail.theirs.is_some());
    let resolved = b
        .client
        .sync_resolve(request(v1::ResolveParams {
            entry_id: shared.id.clone(),
            resolution: v1::Resolution::Mine as i32,
            merged: None,
        }))
        .await
        .unwrap()
        .into_inner();
    assert!(resolved.conflicts.is_empty());
    assert_eq!(
        b.client
            .entries_reveal(request(v1::RevealParams {
                id: shared.id.clone(),
                field: "value".into(),
            }))
            .await
            .unwrap()
            .into_inner()
            .value,
        "B conflict value"
    );
    a.client.sync_run(request(both())).await.unwrap();
    assert_eq!(
        a.client
            .entries_reveal(request(v1::RevealParams {
                id: shared.id.clone(),
                field: "value".into(),
            }))
            .await
            .unwrap()
            .into_inner()
            .value,
        "B conflict value"
    );
    for (resolution, suffix, merged, expected) in [
        (v1::Resolution::Theirs, "theirs", None, "A-theirs"),
        (
            v1::Resolution::Merged,
            "merged",
            Some("combined value"),
            "combined value",
        ),
    ] {
        a.client
            .entries_update(request(v1::EntryUpdateParams {
                id: shared.id.clone(),
                input: Some(secret(&p.id, "shared", &format!("A-{suffix}"))),
            }))
            .await
            .unwrap();
        b.client
            .entries_update(request(v1::EntryUpdateParams {
                id: shared.id.clone(),
                input: Some(secret(&p.id, "shared", &format!("B-{suffix}"))),
            }))
            .await
            .unwrap();
        a.client.sync_run(request(both())).await.unwrap();
        let conflict = b
            .client
            .sync_run(request(both()))
            .await
            .unwrap()
            .into_inner();
        assert!(conflict.conflicts.iter().any(|c| c.entry_id == shared.id));
        let status = b
            .client
            .sync_resolve(request(v1::ResolveParams {
                entry_id: shared.id.clone(),
                resolution: resolution as i32,
                merged: merged.map(|value| secret(&p.id, "shared", value)),
            }))
            .await
            .unwrap()
            .into_inner();
        assert!(status.conflicts.is_empty());
        assert_eq!(
            b.client
                .entries_reveal(request(v1::RevealParams {
                    id: shared.id.clone(),
                    field: "value".into(),
                }))
                .await
                .unwrap()
                .into_inner()
                .value,
            expected
        );
        a.client.sync_run(request(both())).await.unwrap();
        assert_eq!(
            a.client
                .entries_reveal(request(v1::RevealParams {
                    id: shared.id.clone(),
                    field: "value".into(),
                }))
                .await
                .unwrap()
                .into_inner()
                .value,
            expected
        );
    }
}

#[tokio::test]
async fn sync_without_remote_is_a_noop_and_missing_conflict_is_reported() {
    let mut h = Harness::start().await;
    h.init().await;
    let result = h
        .client
        .sync_run(request(both()))
        .await
        .unwrap()
        .into_inner();
    assert_eq!((result.pulled, result.pushed), (0, 0));
    assert!(!result.status.unwrap().has_remote);
    expect_error(
        h.client
            .sync_set_remote(request(v1::SetRemoteParams {
                url: "https://token@example.invalid/repository.git".into(),
            }))
            .await
            .unwrap_err(),
        v1::ErrorCode::InvalidParams,
    );
    let p = h.profile("no remote").await;
    expect_error(
        h.client
            .sync_conflict(request(v1::IdParams { id: p.id }))
            .await
            .unwrap_err(),
        v1::ErrorCode::NotFound,
    );
}
