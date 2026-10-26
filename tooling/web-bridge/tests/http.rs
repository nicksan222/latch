//! Exercise the binary so environment isolation occurs before the runtime starts.
use latch_rpc::{sdk::Call, v1};
use prost::Message;
use serde_json::{Value, json};
use std::{
    process::{Child, Command, Stdio},
    time::Duration,
};
const ORIGIN: &str = "http://localhost:1420";
struct Server {
    child: Child,
    url: String,
}
impl Drop for Server {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = Command::new("/bin/kill")
                .args(["-TERM", &self.child.id().to_string()])
                .status();
        }
        let _ = self.child.wait();
    }
}
async fn server() -> Server {
    server_with_leases(600, 15).await
}
async fn server_with_leases(idle_secs: u64, allocation_secs: u64) -> Server {
    server_with_executable(
        std::path::Path::new(env!("CARGO_BIN_EXE_latch-web-bridge")),
        idle_secs,
        allocation_secs,
    )
    .await
}
async fn server_with_executable(
    executable: &std::path::Path,
    idle_secs: u64,
    allocation_secs: u64,
) -> Server {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let child = Command::new(executable)
        .args([
            "--port",
            &port.to_string(),
            "--idle-secs",
            &idle_secs.to_string(),
            "--allocation-lease-secs",
            &allocation_secs.to_string(),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .env("GIT_CONFIG_GLOBAL", "/definitely/not/a/fixture")
        .spawn()
        .unwrap();
    let server = Server {
        child,
        url: format!("http://127.0.0.1:{port}"),
    };
    for _ in 0..100 {
        if tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .is_ok()
        {
            return server;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("fixture bridge failed to start");
}
async fn create(c: &reqwest::Client, s: &Server, opts: Value) -> Value {
    c.post(format!("{}/__latch/session", s.url))
        .header("Origin", ORIGIN)
        .json(&opts)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap()
}
async fn fresh_session(c: &reqwest::Client, s: &Server) -> Value {
    create(c, s, json!({"fresh":true})).await
}
fn arena_path(created: &Value) -> std::path::PathBuf {
    std::path::Path::new(created["fixture"]["vaults"]["personal"].as_str().unwrap())
        .parent()
        .unwrap()
        .to_owned()
}
fn request(c: &reqwest::Client, s: &Server, id: &str, call: Call) -> reqwest::RequestBuilder {
    c.post(format!("{}/__latch/rpc", s.url))
        .header("Origin", ORIGIN)
        .header("x-latch-session", id)
        .header("x-latch-method", call.method())
        .header("Content-Type", "application/proto")
        .body(call.encode())
}
async fn call<R: Message + Default>(c: &reqwest::Client, s: &Server, id: &str, call: Call) -> R {
    let response = request(c, s, id, call).send().await.unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["content-type"], "application/proto");
    R::decode(response.bytes().await.unwrap()).unwrap()
}
async fn error(c: &reqwest::Client, s: &Server, id: &str, call: Call, code: v1::ErrorCode) {
    let response = request(c, s, id, call).send().await.unwrap();
    assert_eq!(response.status(), 422);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["code"], code as i32);
    assert!(body["message"].as_str().is_some());
}
async fn inspect(c: &reqwest::Client, s: &Server, id: &str) -> Value {
    c.post(format!("{}/__latch/inspect", s.url))
        .header("Origin", ORIGIN)
        .json(&json!({"sessionId":id}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}
async fn remove(c: &reqwest::Client, s: &Server, id: &Value) {
    assert_eq!(
        c.delete(format!("{}/__latch/session", s.url))
            .header("Origin", ORIGIN)
            .json(&json!({"sessionId":id}))
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
}
fn pending_poll(
    c: &reqwest::Client,
    s: &Server,
    created: &Value,
) -> tokio::task::JoinHandle<Result<reqwest::Response, reqwest::Error>> {
    let client = c.clone();
    let url = format!("{}/__latch/events", s.url);
    let id = created["sessionId"].clone();
    tokio::spawn(async move {
        client
            .post(url)
            .header("Origin", ORIGIN)
            .json(&json!({"sessionId":id}))
            .send()
            .await
    })
}
fn reveal(id: &str) -> Call {
    Call::EntriesReveal(v1::RevealParams {
        id: id.into(),
        field: "password".into(),
    })
}
fn sync() -> Call {
    Call::SyncRun(v1::SyncParams {
        mode: v1::SyncMode::Both as i32,
    })
}
async fn scenario(c: &reqwest::Client, s: &Server, id: &str, name: &str) -> reqwest::Response {
    c.post(format!("{}/__latch/sync-scenario", s.url))
        .header("Origin", ORIGIN)
        .json(&json!({"sessionId":id,"scenario":name}))
        .send()
        .await
        .unwrap()
}
#[tokio::test]
async fn isolation_auth_bounds_and_real_vault_cycle() {
    let s = server().await;
    let c = reqwest::Client::new();
    let url = format!("{}/__latch/session", s.url);
    assert_eq!(
        c.post(&url)
            .json(&json!({"fresh":true}))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        c.post(&url)
            .header("Origin", "http://evil.example")
            .json(&json!({}))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        c.get(format!("{}/other", s.url))
            .header("Origin", ORIGIN)
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    assert_eq!(
        c.post(&url)
            .header("Origin", ORIGIN)
            .header("Content-Type", "application/json")
            .body(" ".repeat(2 * 1024 * 1024 + 1))
            .send()
            .await
            .unwrap()
            .status(),
        413
    );
    let seeded = create(&c, &s, json!({"unlocked":true})).await;
    let id = seeded["sessionId"].as_str().unwrap();
    let poll = pending_poll(&c, &s, &seeded);
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert!(
        !poll.is_finished(),
        "fixture bootstrap events escaped into the ready session"
    );
    let _: v1::VaultStatus = call(&c, &s, id, Call::VaultLock(v1::Empty {})).await;
    let response = tokio::time::timeout(Duration::from_secs(2), poll)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let live: Value = response.json().await.unwrap();
    let events = live["events"].as_array().unwrap();
    assert_eq!(events.len(), 1);
    let bytes: Vec<u8> = serde_json::from_value(events[0].clone()).unwrap();
    assert_eq!(
        v1::AgentEvent::decode(bytes.as_slice()).unwrap().kind,
        v1::AgentEventKind::Locked as i32
    );
    let unlock = || {
        Call::VaultUnlock(v1::UnlockParams {
            passphrase: Some(seeded["fixture"]["passphrase"].as_str().unwrap().into()),
            ..Default::default()
        })
    };
    let _: v1::VaultStatus = call(&c, &s, id, unlock()).await;
    let github = seeded["fixture"]["entries"]["acme/GitHub"]
        .as_str()
        .unwrap();
    assert_eq!(github.len(), 26);
    assert_eq!(
        c.get(format!("{}/__latch/health", s.url))
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    let other = fresh_session(&c, &s).await;
    let other_id = other["sessionId"].as_str().unwrap();
    assert_ne!(id, other_id);
    assert_eq!(
        c.post(format!("{}/__latch/rpc", s.url))
            .header("Origin", "http://localhost:1423")
            .header("x-latch-session", id)
            .header("x-latch-method", "VaultStatus")
            .header("Content-Type", "application/proto")
            .body(Vec::new())
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    error(&c, &s, other_id, reveal(github), v1::ErrorCode::NoVault).await;
    let revealed: v1::RevealResult = call(&c, &s, id, reveal(github)).await;
    assert_eq!(revealed.value, "hunter2-acme-Gh!");
    let _: v1::ClipboardCopyResult = call(
        &c,
        &s,
        id,
        Call::ClipboardCopy(v1::ClipboardCopyParams {
            id: github.into(),
            field: "password".into(),
            ..Default::default()
        }),
    )
    .await;
    assert_eq!(inspect(&c, &s, id).await["clipboard"], "hunter2-acme-Gh!");
    assert!(inspect(&c, &s, other_id).await["clipboard"].is_null());
    let history: v1::EntriesHistoryResponse = call(
        &c,
        &s,
        id,
        Call::EntriesHistory(v1::IdParams { id: github.into() }),
    )
    .await;
    assert!(history.items.len() >= 2);
    // Unknown methods, malformed protobuf, and old JSON envelopes fail closed.
    for (method, body) in [
        ("EnvRun", Vec::new()),
        ("vault.status", Vec::new()),
        ("VaultUnlock", vec![0xff]),
    ] {
        let response = c
            .post(format!("{}/__latch/rpc", s.url))
            .header("Origin", ORIGIN)
            .header("x-latch-session", id)
            .header("x-latch-method", method)
            .header("Content-Type", "application/proto")
            .body(body)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 422);
        assert_eq!(
            response.json::<Value>().await.unwrap()["code"],
            v1::ErrorCode::InvalidParams as i32
        );
    }
    assert_eq!(
        c.post(format!("{}/__latch/rpc", s.url))
            .header("Origin", ORIGIN)
            .json(&json!({"sessionId":id,"request":{}}))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    let output = tempfile::tempdir().unwrap();
    let destination = output.path().join("export.json");
    let _: v1::ExportResult = call(
        &c,
        &s,
        id,
        Call::ExportRun(v1::ExportParams {
            format: v1::ExportFormat::Json as i32,
            path: destination.to_string_lossy().into_owned(),
            ..Default::default()
        }),
    )
    .await;
    assert!(!destination.exists());
    assert!(
        inspect(&c, &s, id).await["lastExport"]["content"]
            .as_str()
            .unwrap()
            .contains("hunter2-acme-Gh!")
    );
    let acme = seeded["fixture"]["profiles"]["acme"].as_str().unwrap();
    let _: v1::BrowserSession = call(
        &c,
        &s,
        id,
        Call::BrowserLaunch(v1::BrowserLaunchParams {
            profile_id: acme.into(),
            ..Default::default()
        }),
    )
    .await;
    let _: v1::Empty = call(
        &c,
        &s,
        id,
        Call::BrowserClose(v1::IdParams { id: acme.into() }),
    )
    .await;
    error(
        &c,
        &s,
        id,
        Call::VaultOpen(v1::OpenParams {
            path: "/tmp/../etc".into(),
        }),
        v1::ErrorCode::Denied,
    )
    .await;
    error(
        &c,
        &s,
        id,
        Call::VaultClone(v1::CloneParams {
            url: "https://example.invalid/vault".into(),
            path: "/tmp/elsewhere".into(),
        }),
        v1::ErrorCode::Denied,
    )
    .await;
    let _: v1::VaultStatus = call(&c, &s, id, Call::VaultLock(v1::Empty {})).await;
    error(&c, &s, id, reveal(github), v1::ErrorCode::Locked).await;
    let _: v1::VaultStatus = call(&c, &s, id, unlock()).await;
    let events = pending_poll(&c, &s, &seeded)
        .await
        .unwrap()
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert!(!events["events"].as_array().unwrap().is_empty());
    remove(&c, &s, &seeded["sessionId"]).await;
    assert_eq!(
        c.post(format!("{}/__latch/inspect", s.url))
            .header("Origin", ORIGIN)
            .json(&json!({"sessionId":id}))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
}
#[tokio::test]
async fn idle_session_removes_its_private_arena() {
    let s = server_with_leases(1, 15).await;
    let c = reqwest::Client::new();
    let created = fresh_session(&c, &s).await;
    let arena = arena_path(&created);
    assert!(arena.exists());
    inspect(&c, &s, created["sessionId"].as_str().unwrap()).await;
    tokio::time::sleep(Duration::from_millis(2300)).await;
    assert_eq!(
        c.post(format!("{}/__latch/inspect", s.url))
            .header("Origin", ORIGIN)
            .json(&json!({"sessionId":created["sessionId"]}))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert!(!arena.exists());
}
#[tokio::test]
async fn delete_releases_poll_resources_and_capacity_beyond_session_limit() {
    let s = server().await;
    let c = reqwest::Client::new();
    for _ in 0..36 {
        let created = fresh_session(&c, &s).await;
        let arena = arena_path(&created);
        let poll = pending_poll(&c, &s, &created);
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(!poll.is_finished());
        remove(&c, &s, &created["sessionId"]).await;
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(1), poll)
                .await
                .unwrap()
                .unwrap()
                .unwrap()
                .status(),
            410
        );
        assert!(!arena.exists());
    }
}
#[tokio::test]
async fn unactivated_allocation_expires_but_authenticated_sessions_get_idle_lease() {
    let s = server_with_leases(10, 1).await;
    let c = reqwest::Client::new();
    let lost = fresh_session(&c, &s).await;
    let active = fresh_session(&c, &s).await;
    let inspect_request = |id: Value| {
        c.post(format!("{}/__latch/inspect", s.url))
            .header("Origin", ORIGIN)
            .json(&json!({"sessionId":id}))
    };
    assert_eq!(
        c.post(format!("{}/__latch/inspect", s.url))
            .header("Origin", "http://localhost:1423")
            .json(&json!({"sessionId":lost["sessionId"]}))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        inspect_request(active["sessionId"].clone())
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    tokio::time::sleep(Duration::from_millis(2300)).await;
    assert_eq!(
        inspect_request(lost["sessionId"].clone())
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert!(!arena_path(&lost).exists());
    assert_eq!(
        inspect_request(active["sessionId"].clone())
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert!(arena_path(&active).exists());
}
#[tokio::test]
async fn global_shutdown_wakes_active_poll_and_removes_arena_promptly() {
    let mut s = server().await;
    let c = reqwest::Client::new();
    let created = fresh_session(&c, &s).await;
    let arena = arena_path(&created);
    let poll = pending_poll(&c, &s, &created);
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(!poll.is_finished());
    assert!(
        Command::new("/bin/kill")
            .args(["-TERM", &s.child.id().to_string()])
            .status()
            .unwrap()
            .success()
    );
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if let Some(status) = s.child.try_wait().unwrap() {
                assert!(status.success());
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("shutdown must not wait for event timeout");
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(1), poll)
            .await
            .unwrap()
            .unwrap()
            .unwrap()
            .status(),
        410
    );
    assert!(!arena.exists());
}
#[tokio::test]
async fn named_sync_scenarios_use_real_agent_conflicts_and_clean_up() {
    let s = server().await;
    let c = reqwest::Client::new();
    for name in ["remote_change", "conflict"] {
        let created = create(&c, &s, json!({"unlocked":true})).await;
        let id = created["sessionId"].as_str().unwrap();
        let arena = arena_path(&created);
        let ready = scenario(&c, &s, id, name).await;
        assert_eq!(ready.status(), 200);
        let ready: Value = ready.json().await.unwrap();
        let entry = ready["entryId"].as_str().unwrap();
        let synced: v1::SyncResult = call(&c, &s, id, sync()).await;
        if name == "conflict" {
            assert_eq!(synced.conflicts[0].entry_id, entry);
            let detail: v1::ConflictDetail = call(
                &c,
                &s,
                id,
                Call::SyncConflict(v1::IdParams { id: entry.into() }),
            )
            .await;
            let password = |e: v1::Entry| match e.payload.unwrap().data.unwrap() {
                v1::entry_payload::Data::Login(l) => l.password.unwrap(),
                _ => panic!("login expected"),
            };
            assert_eq!(password(detail.mine.unwrap()), "fixture-mine-password");
            assert_eq!(password(detail.theirs.unwrap()), "fixture-remote-password");
            let resolved: v1::SyncStatus = call(
                &c,
                &s,
                id,
                Call::SyncResolve(v1::ResolveParams {
                    entry_id: entry.into(),
                    resolution: v1::Resolution::Theirs as i32,
                    merged: None,
                }),
            )
            .await;
            assert!(resolved.conflicts.is_empty());
            assert_eq!(resolved.ahead, 0);
        } else {
            assert_eq!(synced.pulled, 1);
            assert!(synced.conflicts.is_empty());
        }
        let revealed: v1::RevealResult = call(&c, &s, id, reveal(entry)).await;
        assert_eq!(revealed.value, "fixture-remote-password");
        let doctor: v1::DoctorReport =
            call(&c, &s, id, Call::DoctorRun(v1::DoctorParams::default())).await;
        assert!(doctor.ok);
        for forbidden in [
            Call::VaultLocate(v1::OpenParams {
                path: "/etc".into(),
            }),
            Call::DoctorScan(v1::OpenParams {
                path: "/etc".into(),
            }),
            Call::HooksInstall(v1::HookParams {
                path: "/etc".into(),
                executable: Some("/bin/false".into()),
            }),
            Call::HooksUninstall(v1::HookParams {
                path: "/etc".into(),
                executable: None,
            }),
            Call::SyncSetRemote(v1::SetRemoteParams { url: "/etc".into() }),
            Call::SyncSetRemote(v1::SetRemoteParams {
                url: "https://token@example.invalid/vault".into(),
            }),
            Call::VaultClone(v1::CloneParams {
                url: "https://example.invalid".into(),
                path: "/tmp/outside".into(),
            }),
        ] {
            error(&c, &s, id, forbidden, v1::ErrorCode::Denied).await;
        }
        assert_eq!(scenario(&c, &s, id, name).await.status(), 422);
        assert_eq!(scenario(&c, &s, id, "unknown").await.status(), 400);
        assert!(
            c.post(format!("{}/__latch/sync-scenario", s.url))
                .header("Origin", ORIGIN)
                .json(&json!({"sessionId":id,"scenario":"conflict","path":"/etc"}))
                .send()
                .await
                .unwrap()
                .status()
                .is_client_error()
        );
        let _: v1::VaultStatus = call(
            &c,
            &s,
            id,
            Call::VaultOpen(v1::OpenParams {
                path: created["fixture"]["vaults"]["employer"]
                    .as_str()
                    .unwrap()
                    .into(),
            }),
        )
        .await;
        error(&c, &s, id, sync(), v1::ErrorCode::Denied).await;
        remove(&c, &s, &created["sessionId"]).await;
        assert!(!arena.exists());
    }
}
#[tokio::test]
async fn recipients_allow_native_age_only_and_sync_requires_named_setup() {
    let s = server().await;
    let c = reqwest::Client::new();
    let created = create(&c, &s, json!({"unlocked":true})).await;
    let id = created["sessionId"].as_str().unwrap();
    error(&c, &s, id, sync(), v1::ErrorCode::Denied).await;
    for key in [
        "age1invalid".to_owned(),
        format!("age1{}", "q".repeat(58)),
        "ssh-ed25519 invalid".to_owned(),
        "ssh-rsa $(touch /tmp/not-allowed)".to_owned(),
        "age1yubikey1invalid".to_owned(),
        format!("age1plugin1{}", "q".repeat(51)),
    ] {
        error(
            &c,
            &s,
            id,
            Call::RecipientsAdd(v1::AddRecipientParams {
                label: "test".into(),
                public_key: key,
            }),
            v1::ErrorCode::Denied,
        )
        .await;
    }
    let added: v1::RecipientsAddResponse = call(
        &c,
        &s,
        id,
        Call::RecipientsAdd(v1::AddRecipientParams {
            label: "fixture-key".into(),
            public_key: created["fixture"]["recipientPublicKey"]
                .as_str()
                .unwrap()
                .into(),
        }),
    )
    .await;
    assert!(added.items.iter().any(|r| r.label == "fixture-key"));
    let _: v1::RecipientsRemoveResponse = call(
        &c,
        &s,
        id,
        Call::RecipientsRemove(v1::RemoveRecipientParams {
            label: "fixture-key".into(),
        }),
    )
    .await;
}
#[tokio::test]
async fn failed_sync_setup_cannot_retry_or_enable_sync() {
    let s = server().await;
    let c = reqwest::Client::new();
    let created = create(&c, &s, json!({"unlocked":true})).await;
    let id = created["sessionId"].as_str().unwrap();
    let _: v1::Empty = call(
        &c,
        &s,
        id,
        Call::EntriesDelete(v1::IdParams {
            id: created["fixture"]["entries"]["acme/GitHub"]
                .as_str()
                .unwrap()
                .into(),
        }),
    )
    .await;
    assert_eq!(scenario(&c, &s, id, "conflict").await.status(), 422);
    let peer = arena_path(&created).join("sync-peer");
    assert!(peer.exists());
    std::fs::remove_dir_all(&peer).unwrap();
    assert_eq!(scenario(&c, &s, id, "conflict").await.status(), 422);
    assert!(!peer.exists());
    error(&c, &s, id, sync(), v1::ErrorCode::Denied).await;
    error(
        &c,
        &s,
        id,
        Call::SyncSetRemote(v1::SetRemoteParams {
            url: "https://example.invalid/vault".into(),
        }),
        v1::ErrorCode::Denied,
    )
    .await;
}

#[tokio::test]
async fn json_extractor_failures_do_not_echo_request_values() {
    let s = server().await;
    let c = reqwest::Client::new();
    let marker = "sensitive-fixture-marker";
    let response = c
        .post(format!("{}/__latch/session", s.url))
        .header("Origin", ORIGIN)
        .json(&json!({"latencyMs": marker}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 422);
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert!(response.text().await.unwrap().is_empty());
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn running_bridge_survives_executable_replacement() {
    let directory = tempfile::tempdir().unwrap();
    let executable = directory.path().join("latch-web-bridge");
    let replacement = directory.path().join("replacement");
    std::fs::copy(env!("CARGO_BIN_EXE_latch-web-bridge"), &executable).unwrap();
    let server = server_with_executable(&executable, 600, 15).await;
    // Cargo replaces binaries while other shared-checkout dev/test runs may still use them.
    std::fs::copy(&executable, &replacement).unwrap();
    std::fs::rename(&replacement, &executable).unwrap();
    let client = reqwest::Client::new();
    let created = create(&client, &server, json!({"unlocked":true})).await;
    let id = created["sessionId"].as_str().unwrap();
    let session: v1::BrowserSession = call(
        &client,
        &server,
        id,
        Call::BrowserLaunch(v1::BrowserLaunchParams {
            profile_id: created["fixture"]["profiles"]["acme"]
                .as_str()
                .unwrap()
                .into(),
            ..Default::default()
        }),
    )
    .await;
    assert_ne!(session.pid, 0);
    remove(&client, &server, &created["sessionId"]).await;
}
