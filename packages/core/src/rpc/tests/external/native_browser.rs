//! A real managed runtime, driven only by a generated external gRPC client.
use super::*;
use latch_rpc::v1;
use std::time::Duration;

#[tokio::test]
#[ignore = "downloads and launches the pinned managed Firefox; run bun run test:browser:native"]
async fn managed_firefox_imports_updates_deletes_and_closes_on_lock() {
    let mut h = Harness::start_native().await;
    h.init().await;
    let p = h.profile("Native browser").await;
    let other = h.profile("Other workspace").await;
    let input = |profile_id: String, username: &str| v1::EntryInput {
        profile_id,
        name: "Local login".into(),
        payload: Some(v1::EntryPayload {
            data: Some(v1::entry_payload::Data::Login(v1::LoginData {
                username: username.into(),
                password: Some(SECRET.into()),
                urls: vec!["https://example.invalid/login".into()],
                ..Default::default()
            })),
        }),
        ..Default::default()
    };
    h.client
        .entries_create(request(input(other.id, "other-workspace-user")))
        .await
        .unwrap();
    let entry = h
        .client
        .entries_create(request(input(p.id.clone(), "fixture-user")))
        .await
        .unwrap()
        .into_inner();
    let mut launch = request(v1::BrowserLaunchParams {
        profile_id: p.id.clone(),
        url: None,
    });
    launch.set_timeout(Duration::from_secs(240));
    let opened = h.client.browser_launch(launch).await.unwrap().into_inner();
    assert_eq!(opened.engine, v1::BrowserEngine::Firefox as i32);
    let profile = std::path::Path::new(&opened.user_data_dir);
    let owned: Vec<String> =
        serde_json::from_slice(&std::fs::read(profile.join("latch-login-ownership.json")).unwrap())
            .unwrap();
    assert_eq!(owned.len(), 1, "another workspace must not be imported");
    assert!(!profile.join("latch-credentials.json").exists());
    h.client
        .entries_update(request(v1::EntryUpdateParams {
            id: entry.id.clone(),
            input: Some(v1::EntryInput {
                name: "Updated login".into(),
                ..input(p.id.clone(), "fixture-user")
            }),
        }))
        .await
        .unwrap();
    h.client
        .entries_delete(request(v1::IdParams { id: entry.id }))
        .await
        .unwrap();
    let owned: Vec<String> =
        serde_json::from_slice(&std::fs::read(profile.join("latch-login-ownership.json")).unwrap())
            .unwrap();
    assert!(owned.is_empty());
    h.client.vault_lock(request(v1::Empty {})).await.unwrap();
    assert!(
        h.client
            .browser_list(request(v1::Empty {}))
            .await
            .unwrap()
            .into_inner()
            .items
            .is_empty()
    );
    assert!(
        !profile.exists(),
        "lock must remove decrypted browser state"
    );
    h.client
        .vault_unlock(request(v1::UnlockParams {
            passphrase: Some(PASS.into()),
            ..Default::default()
        }))
        .await
        .unwrap();
    let info = h
        .client
        .browser_session_info(request(v1::IdParams { id: p.id }))
        .await
        .unwrap()
        .into_inner();
    assert!(info.exists);
}
