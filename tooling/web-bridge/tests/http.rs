//! Exercise the binary so environment isolation occurs before the runtime starts.
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
    server_with_idle(600).await
}
async fn server_with_idle(idle_secs: u64) -> Server {
    server_with_leases(idle_secs, 15).await
}
async fn server_with_leases(idle_secs: u64, allocation_secs: u64) -> Server {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let child = Command::new(env!("CARGO_BIN_EXE_latch-web-bridge"))
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
async fn call(c: &reqwest::Client, s: &Server, id: &str, method: &str, params: Value) -> Value {
    let r = c
        .post(format!("{}/__latch/rpc", s.url))
        .header("Origin", ORIGIN)
        .json(&json!({"sessionId":id,"request":{"id":7,"method":method,"params":params}}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    r.json().await.unwrap()
}
#[tokio::test]
async fn isolation_auth_bounds_and_real_vault_cycle() {
    let s = server().await;
    let c = reqwest::Client::new();
    let create = format!("{}/__latch/session", s.url);
    assert_eq!(
        c.post(&create)
            .json(&json!({"fresh":true}))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        c.post(&create)
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
        c.post(&create)
            .header("Origin", ORIGIN)
            .header("Content-Type", "application/json")
            .body(" ".repeat(2 * 1024 * 1024 + 1))
            .send()
            .await
            .unwrap()
            .status(),
        413
    );
    let seeded: Value = c
        .post(&create)
        .header("Origin", ORIGIN)
        .json(&json!({"unlocked":true}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let id = seeded["sessionId"].as_str().unwrap();
    // Bootstrap fixture notifications must not reach a ready client. Its
    // first poll waits until an actual post-readiness mutation occurs.
    let event_client = c.clone();
    let event_url = format!("{}/__latch/events", s.url);
    let event_id = id.to_owned();
    let first_poll = tokio::spawn(async move {
        event_client
            .post(event_url)
            .header("Origin", ORIGIN)
            .json(&json!({"sessionId":event_id}))
            .send()
            .await
            .unwrap()
            .json::<Value>()
            .await
            .unwrap()
    });
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert!(
        !first_poll.is_finished(),
        "fixture bootstrap events escaped into the ready session"
    );
    let locked = call(&c, &s, id, "vault.lock", json!({})).await;
    assert!(locked["response"]["error"].is_null());
    let live = tokio::time::timeout(Duration::from_secs(2), first_poll)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(live["events"].as_array().unwrap().len(), 1);
    assert_eq!(live["events"][0]["kind"], "locked");
    let unlocked=call(&c,&s,id,"vault.unlock",json!({"passphrase":seeded["fixture"]["passphrase"],"identityFile":null,"timeoutSecs":null})).await;
    assert!(unlocked["response"]["error"].is_null());
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
    let other: Value = c
        .post(&create)
        .header("Origin", ORIGIN)
        .json(&json!({"fresh":true}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let other_id = other["sessionId"].as_str().unwrap();
    assert_ne!(id, other_id);
    assert_eq!(
        c.post(format!("{}/__latch/rpc", s.url))
            .header("Origin", "http://localhost:1423")
            .json(&json!({"sessionId":id,"request":{"id":1,"method":"vault.status","params":{}}}))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    let missing = call(
        &c,
        &s,
        other_id,
        "entries.reveal",
        json!({"id":github,"field":"password"}),
    )
    .await;
    assert!(missing["response"]["error"].is_object());
    let revealed = call(
        &c,
        &s,
        id,
        "entries.reveal",
        json!({"id":github,"field":"password"}),
    )
    .await;
    assert!(
        revealed["response"]["error"].is_null(),
        "{}",
        revealed["response"]["error"]
    );
    assert_eq!(revealed["response"]["result"]["value"], "hunter2-acme-Gh!");
    let copy = call(
        &c,
        &s,
        id,
        "clipboard.copy",
        json!({"id":github,"field":"password"}),
    )
    .await;
    assert_eq!(copy["inspection"]["clipboard"], "hunter2-acme-Gh!");
    let isolated: Value = c
        .post(format!("{}/__latch/inspect", s.url))
        .header("Origin", ORIGIN)
        .json(&json!({"sessionId":other_id}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(isolated["clipboard"].is_null());
    let history = call(&c, &s, id, "entries.history", json!({"id":github})).await;
    assert!(history["response"]["result"].as_array().unwrap().len() >= 2);
    let acme = &seeded["fixture"]["profiles"]["acme"];
    let run = call(
        &c,
        &s,
        id,
        "env.run",
        json!({"profileId":acme,"entryIds":null,"command":["touch","/not-allowed"]}),
    )
    .await;
    assert_eq!(run["response"]["error"]["code"], "invalid_params");
    assert_eq!(run["response"]["error"]["message"], "unknown RPC procedure");
    let output = tempfile::tempdir().unwrap();
    let destination = output.path().join("export.json");
    let export = call(
        &c,
        &s,
        id,
        "export.run",
        json!({"format":"json","path":destination,"profileId":null,"passphrase":null}),
    )
    .await;
    assert!(
        export["response"]["error"].is_null(),
        "{}",
        export["response"]["error"]
    );
    assert!(!destination.exists());
    assert!(
        export["inspection"]["lastExport"]["content"]
            .as_str()
            .unwrap()
            .contains("hunter2-acme-Gh!")
    );
    let browser = call(
        &c,
        &s,
        id,
        "browser.launch",
        json!({"profileId":acme,"engine":null,"url":null}),
    )
    .await;
    assert!(
        browser["response"]["error"].is_null(),
        "{}",
        browser["response"]["error"]
    );
    let close = call(&c, &s, id, "browser.close", json!({"id":acme})).await;
    assert!(
        close["response"]["error"].is_null(),
        "{}",
        close["response"]["error"]
    );
    let denied = call(&c, &s, id, "vault.open", json!({"path":"/tmp/../etc"})).await;
    assert_eq!(denied["response"]["error"]["code"], "denied");
    let denied = call(
        &c,
        &s,
        id,
        "vault.clone",
        json!({"url":"https://example.invalid/vault","path":"/tmp/elsewhere"}),
    )
    .await;
    assert_eq!(denied["response"]["error"]["code"], "denied");
    let locked = call(&c, &s, id, "vault.lock", json!({})).await;
    assert!(locked["response"]["error"].is_null());
    let locked = call(
        &c,
        &s,
        id,
        "entries.reveal",
        json!({"id":github,"field":"password"}),
    )
    .await;
    assert_eq!(locked["response"]["error"]["code"], "locked");
    let unlocked=call(&c,&s,id,"vault.unlock",json!({"passphrase":seeded["fixture"]["passphrase"],"identityFile":null,"timeoutSecs":null})).await;
    assert!(unlocked["response"]["error"].is_null());
    let events: Value = c
        .post(format!("{}/__latch/events", s.url))
        .header("Origin", ORIGIN)
        .json(&json!({"sessionId":id}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(!events["events"].as_array().unwrap().is_empty());
    assert_eq!(
        c.delete(&create)
            .header("Origin", ORIGIN)
            .json(&json!({"sessionId":id}))
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
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
    let s = server_with_idle(1).await;
    let c = reqwest::Client::new();
    let created: Value = c
        .post(format!("{}/__latch/session", s.url))
        .header("Origin", ORIGIN)
        .json(&json!({"fresh":true}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let vault = std::path::Path::new(created["fixture"]["vaults"]["personal"].as_str().unwrap());
    let arena = vault.parent().unwrap();
    assert!(arena.exists());
    assert_eq!(
        c.post(format!("{}/__latch/inspect", s.url))
            .header("Origin", ORIGIN)
            .json(&json!({"sessionId":created["sessionId"]}))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
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

async fn fresh_session(c: &reqwest::Client, s: &Server) -> Value {
    c.post(format!("{}/__latch/session", s.url))
        .header("Origin", ORIGIN)
        .json(&json!({"fresh":true}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap()
}
fn arena_path(created: &Value) -> std::path::PathBuf {
    std::path::Path::new(created["fixture"]["vaults"]["personal"].as_str().unwrap())
        .parent()
        .unwrap()
        .to_owned()
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
#[tokio::test]
async fn delete_releases_poll_resources_and_capacity_beyond_session_limit() {
    let s = server().await;
    let c = reqwest::Client::new();
    for _ in 0..36 {
        let created = fresh_session(&c, &s).await;
        let arena = arena_path(&created);
        let poll = pending_poll(&c, &s, &created);
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(
            !poll.is_finished(),
            "fresh session poll should be waiting for live events"
        );
        let deleted = c
            .delete(format!("{}/__latch/session", s.url))
            .header("Origin", ORIGIN)
            .json(&json!({"sessionId":created["sessionId"]}))
            .send()
            .await
            .unwrap();
        assert_eq!(deleted.status(), 204);
        let response = tokio::time::timeout(Duration::from_secs(1), poll)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(
            response.status(),
            410,
            "an accepted event poll must be woken by deletion"
        );
        assert!(
            !arena.exists(),
            "DELETE must release the arena before returning"
        );
    }
}
#[tokio::test]
async fn unactivated_allocation_expires_but_authenticated_sessions_get_idle_lease() {
    let s = server_with_leases(10, 1).await;
    let c = reqwest::Client::new();
    let lost = fresh_session(&c, &s).await;
    let lost_arena = arena_path(&lost);
    let active = fresh_session(&c, &s).await;
    let active_arena = arena_path(&active);
    let inspect = |id: Value| {
        c.post(format!("{}/__latch/inspect", s.url))
            .header("Origin", ORIGIN)
            .json(&json!({"sessionId":id}))
    };
    // A wrong origin must not activate an allocation whose response was lost.
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
        inspect(active["sessionId"].clone())
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    tokio::time::sleep(Duration::from_millis(2300)).await;
    assert_eq!(
        inspect(lost["sessionId"].clone())
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert!(!lost_arena.exists());
    assert_eq!(
        inspect(active["sessionId"].clone())
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert!(active_arena.exists());
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
    let stopped = Command::new("/bin/kill")
        .args(["-TERM", &s.child.id().to_string()])
        .status()
        .unwrap();
    assert!(stopped.success());
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
    .expect("shutdown must not wait for the 20 second event timeout");
    let response = tokio::time::timeout(Duration::from_secs(1), poll)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(response.status(), 410);
    assert!(
        !arena.exists(),
        "shutdown must remove worker arenas before exiting"
    );
}

// Removing the named fixture handler or broadening its arena boundary must fail
// these tests: sync uses two real encrypted clones, never a caller-chosen remote.
#[tokio::test]
async fn named_sync_scenarios_use_real_agent_conflicts_and_clean_up() {
    let s = server().await;
    let c = reqwest::Client::new();
    for scenario in ["remote_change", "conflict"] {
        let created: Value = c
            .post(format!("{}/__latch/session", s.url))
            .header("Origin", ORIGIN)
            .json(&json!({"unlocked":true}))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap();
        let id = created["sessionId"].as_str().unwrap();
        let arena = arena_path(&created);
        let ready = call(
            &c,
            &s,
            id,
            "fixture.syncScenario",
            json!({"scenario":scenario}),
        )
        .await;
        assert!(ready["response"]["error"].is_null(), "{ready}");
        let entry = &ready["response"]["result"]["entryId"];
        assert!(entry.as_str().is_some());
        let synced = call(&c, &s, id, "sync.run", json!({"mode":"both"})).await;
        assert!(synced["response"]["error"].is_null(), "{synced}");
        if scenario == "conflict" {
            assert_eq!(
                synced["response"]["result"]["conflicts"][0]["entryId"],
                *entry
            );
            let detail = call(&c, &s, id, "sync.conflict", json!({"id":entry})).await;
            assert_eq!(
                detail["response"]["result"]["mine"]["payload"]["data"]["password"],
                "fixture-mine-password"
            );
            assert_eq!(
                detail["response"]["result"]["theirs"]["payload"]["data"]["password"],
                "fixture-remote-password"
            );
            let resolved = call(
                &c,
                &s,
                id,
                "sync.resolve",
                json!({"entryId":entry,"resolution":"theirs","merged":null}),
            )
            .await;
            assert!(resolved["response"]["error"].is_null(), "{resolved}");
            assert_eq!(resolved["response"]["result"]["conflicts"], json!([]));
            assert_eq!(resolved["response"]["result"]["ahead"], 0);
        } else {
            assert_eq!(synced["response"]["result"]["pulled"], 1);
            assert_eq!(synced["response"]["result"]["conflicts"], json!([]));
        }
        let revealed = call(
            &c,
            &s,
            id,
            "entries.reveal",
            json!({"id":entry,"field":"password"}),
        )
        .await;
        assert_eq!(
            revealed["response"]["result"]["value"],
            "fixture-remote-password"
        );
        let doctor = call(
            &c,
            &s,
            id,
            "doctor.run",
            json!({"fix":false,"rebuildManifest":false,"staged":false}),
        )
        .await;
        assert_eq!(doctor["response"]["result"]["ok"], true, "{doctor}");
        for (method, params) in [
            ("vault.locate", json!({"path":"/etc"})),
            ("doctor.scan", json!({"path":"/etc"})),
            (
                "hooks.install",
                json!({"path":"/etc", "executable":"/bin/false"}),
            ),
            ("hooks.uninstall", json!({"path":"/etc"})),
            ("sync.setRemote", json!({"url":"/etc"})),
            (
                "sync.setRemote",
                json!({"url":"https://token@example.invalid/vault"}),
            ),
            ("fixture.syncScenario", json!({"scenario":scenario})),
            (
                "vault.clone",
                json!({"url":"https://example.invalid","path":"/tmp/outside"}),
            ),
            (
                "fixture.syncScenario",
                json!({"scenario":"conflict","path":"/etc"}),
            ),
            ("fixture.syncScenario", json!({"scenario":"unknown"})),
        ] {
            assert_eq!(
                call(&c, &s, id, method, params).await["response"]["error"]["code"],
                "denied"
            );
        }
        call(
            &c,
            &s,
            id,
            "vault.open",
            json!({"path":created["fixture"]["vaults"]["employer"]}),
        )
        .await;
        assert_eq!(
            call(&c, &s, id, "sync.run", json!({"mode":"both"})).await["response"]["error"]["code"],
            "denied"
        );
        c.delete(format!("{}/__latch/session", s.url))
            .header("Origin", ORIGIN)
            .json(&json!({"sessionId":id}))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
        assert!(!arena.exists());
    }
}

#[tokio::test]
async fn recipients_allow_native_age_only_and_sync_requires_named_setup() {
    let s = server().await;
    let c = reqwest::Client::new();
    let created: Value = c
        .post(format!("{}/__latch/session", s.url))
        .header("Origin", ORIGIN)
        .json(&json!({"unlocked":true}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let id = created["sessionId"].as_str().unwrap();
    assert_eq!(
        call(&c, &s, id, "sync.run", json!({"mode":"both"})).await["response"]["error"]["code"],
        "denied"
    );
    for key in [
        "age1invalid".to_owned(),
        format!("age1{}", "q".repeat(58)),
        "ssh-ed25519 invalid".to_owned(),
        "ssh-rsa $(touch /tmp/not-allowed)".to_owned(),
        "age1yubikey1invalid".to_owned(),
        format!("age1plugin1{}", "q".repeat(51)),
    ] {
        assert_eq!(
            call(
                &c,
                &s,
                id,
                "recipients.add",
                json!({"label":"test","publicKey":key})
            )
            .await["response"]["error"]["code"],
            "denied"
        );
    }
    let added = call(
        &c,
        &s,
        id,
        "recipients.add",
        json!({"label":"fixture-key","publicKey":created["fixture"]["recipientPublicKey"]}),
    )
    .await;
    assert!(added["response"]["error"].is_null(), "{added}");
    assert!(
        added["response"]["result"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["label"] == "fixture-key")
    );
    let removed = call(
        &c,
        &s,
        id,
        "recipients.remove",
        json!({"label":"fixture-key"}),
    )
    .await;
    assert!(removed["response"]["error"].is_null(), "{removed}");
}

#[tokio::test]
async fn failed_sync_setup_cannot_retry_or_enable_sync() {
    let s = server().await;
    let c = reqwest::Client::new();
    let created: Value = c
        .post(format!("{}/__latch/session", s.url))
        .header("Origin", ORIGIN)
        .json(&json!({"unlocked":true}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let id = created["sessionId"].as_str().unwrap();
    let deleted = call(
        &c,
        &s,
        id,
        "entries.delete",
        json!({"id":created["fixture"]["entries"]["acme/GitHub"]}),
    )
    .await;
    assert!(deleted["response"]["error"].is_null(), "{deleted}");
    let first = call(
        &c,
        &s,
        id,
        "fixture.syncScenario",
        json!({"scenario":"conflict"}),
    )
    .await;
    assert_eq!(first["response"]["error"]["code"], "denied");
    let peer = arena_path(&created).join("sync-peer");
    assert!(
        peer.exists(),
        "setup must have failed after cloning the peer"
    );
    // Make another setup attempt observable: a retry would clone this again.
    std::fs::remove_dir_all(&peer).unwrap();
    let second = call(
        &c,
        &s,
        id,
        "fixture.syncScenario",
        json!({"scenario":"conflict"}),
    )
    .await;
    assert_eq!(second["response"]["error"]["code"], "denied");
    assert!(
        !peer.exists(),
        "failed setup must never repeat its Git mutations"
    );
    for (method, params) in [
        ("sync.run", json!({"mode":"both"})),
        (
            "sync.setRemote",
            json!({"url":"https://example.invalid/vault"}),
        ),
    ] {
        assert_eq!(
            call(&c, &s, id, method, params).await["response"]["error"]["code"],
            "denied"
        );
    }
}
