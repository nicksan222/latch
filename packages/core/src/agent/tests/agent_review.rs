// Regression tests for review findings at the agent level: lifecycle
// generation (lock during unlock), browser close off the timer thread,
// trust/integrity status fields, `vault.trust`/`vault.forgetTrust`,
// effective settings, remote URL credentials, unlock timeout bounds, start
// page validation.

#[path = "../../../tests/agent_support/mod.rs"]
mod agent_support;

use std::io::Write;
use std::time::{Duration, Instant};

use agent_support::methods as m;
use agent_support::*;
use latch::agent::clipboard::ClipboardBackend;
use latch::core::crypto;
use latch::protocol::*;
use latch_automation::fixtures::{self, BrowserConfig, FixtureConfig};
use latch_rpc::sdk::Call;
use serde_json::json;

#[test]
fn lock_cancels_in_flight_unlock() {
    let a = TestAgent::new();
    let res: InitResult = a.ok(
        m::VAULT_INIT,
        json!({
            "path": a.vault_path().display().to_string(),
            "passphrase": PASS, "kdf": crypto::WEAK_KDF,
            "gitName": "Alice", "gitEmail": "alice@example.com",
            "remoteUrl": null, "recoveryKit": true,
        }),
    );
    a.agent.lock();
    // The identity file is a FIFO: the unlock blocks reading it until we
    // write, so the lock below happens while the unlock is in flight.
    let fifo = a.dir.path().join("identity.fifo");
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success()
    );
    let agent = a.agent.clone();
    let f2 = fifo.clone();
    let th = std::thread::spawn(move || {
        agent.execute(
            &latch::agent::ClientCtx::cli(),
            Call::VaultUnlock(
                UnlockParams {
                    passphrase: None,
                    identity_file: Some(f2.display().to_string()),
                    timeout_secs: None,
                }
                .into(),
            ),
        )
    });
    let mut w = std::fs::OpenOptions::new().write(true).open(&fifo).unwrap();
    a.agent.lock();
    writeln!(w, "{}", res.recovery_kit.unwrap().recovery_secret_key).unwrap();
    drop(w);
    let r = th.join().unwrap();
    assert_eq!(r.unwrap_err().code, ErrorCode::Conflict);
    assert!(
        !a.agent.is_unlocked(),
        "a lock must win over an in-flight unlock"
    );
    let st: VaultStatus = a.ok(m::VAULT_STATUS, json!(null));
    assert!(st.locked);
    // A fresh unlock works.
    a.unlock();
    assert!(a.agent.is_unlocked());
}

#[test]
fn browser_close_does_not_block_clipboard_expiry() {
    let a = TestAgent::with(|o| {
        o.browser_lock_grace = Duration::ZERO;
        o.tick = Duration::from_millis(20);
        let root = o.config_dir.as_ref().unwrap().parent().unwrap();
        o.browser_config = Some(latch::agent::browser::ManagerConfig {
            ram_dir: root.join("ram"),
            data_dir: root.join("data"),
            browser_override: Some(root.join("fake-browser")),
            term_timeout: Duration::from_secs(2),
            archive_wait: Duration::from_secs(3),
        });
    });
    a.init();
    let bin = a.dir.path().join("fake-browser");
    let ready = a.dir.path().join("ready");
    fixtures::install(
        FixtureConfig::Browser(BrowserConfig {
            ignore_term: true,
            ready_path: Some(ready.clone()),
            ..BrowserConfig::default()
        }),
        &bin,
    )
    .unwrap();
    let input = profile_input("x");
    let p: Profile = a.ok(m::PROFILES_CREATE, serde_json::to_value(&input).unwrap());
    let e: Entry = a.ok(
        m::ENTRIES_CREATE,
        serde_json::to_value(EntryInput {
            profile_id: p.id.clone(),
            name: "secret".into(),
            tags: vec![],
            notes: String::new(),
            favorite: false,
            payload: EntryPayload::Secret(SecretData {
                value: Some("clip-secret".into()),
            }),
        })
        .unwrap(),
    );
    let _: BrowserSession = a.ok(m::BROWSER_LAUNCH, json!({ "profileId": p.id }));
    assert!(wait_until(Duration::from_secs(5), || ready.exists()));
    let _: ClipboardCopyResult = a.ok(
        m::CLIPBOARD_COPY,
        json!({ "id": e.id, "field": "value", "clearAfterSecs": 2 }),
    );
    let mut settings: Settings = a.ok(m::SETTINGS_GET, json!({}));
    settings.auto_lock_secs = 1;
    let _: Settings = a.ok(m::SETTINGS_UPDATE, serde_json::to_value(settings).unwrap());
    // Auto-lock must not block the clipboard timer while it waits for a
    // browser that ignores TERM to be killed and archived.
    assert!(wait_until(Duration::from_millis(2500), || a
        .clip
        .get_text()
        .is_none()));
    assert!(wait_until(Duration::from_secs(20), || a
        .agent
        .browsers()
        .list()
        .is_empty()));
}

#[test]
fn status_reports_trust_and_forget_trust_requires_lock() {
    let a = TestAgent::new();
    a.init();
    let st: VaultStatus = a.ok(m::VAULT_STATUS, json!(null));
    let t = st.trust.clone().expect("trust status");
    assert!(!t.fingerprint.is_empty());
    assert!(st.integrity_problems.is_empty());
    assert!(!st.rotation_pending);
    let t2: TrustStatus = a.ok(m::VAULT_TRUST, json!({}));
    assert_eq!(t2.fingerprint, t.fingerprint);
    // Unlocked: refused without confirm.
    assert_eq!(a.err(m::VAULT_FORGET_TRUST, json!({})), ErrorCode::Conflict);
    assert!(a.agent.is_unlocked());
    // With confirm: locks, forgets; the next unlock is a first use.
    let t3: TrustStatus = a.ok(m::VAULT_FORGET_TRUST, json!({ "confirm": true }));
    assert!(!a.agent.is_unlocked());
    assert_eq!(t3.state, VaultTrustState::FirstUse);
    let st = a.unlock();
    let t4 = st.trust.unwrap();
    assert_eq!(t4.state, VaultTrustState::FirstUse);
    assert_eq!(t4.fingerprint, t.fingerprint);
    // Locked status still carries the trust (read from the keychain).
    a.agent.lock();
    let st: VaultStatus = a.ok(m::VAULT_STATUS, json!(null));
    assert_eq!(st.trust.unwrap().fingerprint, t.fingerprint);
    let _: TrustStatus = a.ok(m::VAULT_FORGET_TRUST, json!({}));
}

#[test]
fn settings_report_effective_values_and_record_consent() {
    let a = TestAgent::new();
    a.init();
    let mut s: Settings = a.ok(m::SETTINGS_GET, json!(null));
    assert!(!s.names_in_commits);
    s.names_in_commits = true;
    let s2: Settings = a.ok(m::SETTINGS_UPDATE, serde_json::to_value(&s).unwrap());
    assert!(s2.names_in_commits);
    // Withdraw this clone's consent behind the agent's back: config.toml
    // still says true, but the effective value (what settings.get reports
    // and what commits use) is false.
    let vault = latch::core::Vault::open(a.vault_path()).unwrap();
    assert!(vault.config().sync.names_in_commits);
    latch::core::trust::set_consent(vault.id(), vault.path(), latch::core::Consent::default())
        .unwrap();
    a.agent.lock();
    a.unlock();
    let s3: Settings = a.ok(m::SETTINGS_GET, json!(null));
    assert!(!s3.names_in_commits);
    a.agent.lock();
    let s4: Settings = a.ok(m::SETTINGS_GET, json!(null));
    assert!(!s4.names_in_commits, "locked: effective value too");
    a.unlock();
    // Updating to true records consent again.
    let mut s5 = s3.clone();
    s5.names_in_commits = true;
    let s6: Settings = a.ok(m::SETTINGS_UPDATE, serde_json::to_value(&s5).unwrap());
    assert!(s6.names_in_commits);
}

#[test]
fn remote_urls_with_credentials_are_rejected_and_redacted() {
    let a = TestAgent::new();
    a.init();
    let r = a
        .call(
            m::SYNC_SET_REMOTE,
            json!({ "url": "https://user:sentinel-token@example.test/repo.git" }),
        )
        .unwrap_err();
    assert_eq!(r.code, ErrorCode::InvalidParams);
    assert!(!r.message.contains("sentinel-token"));
    assert_eq!(
        a.err(
            m::SYNC_SET_REMOTE,
            json!({ "url": "--upload-pack=touch /tmp/x" })
        ),
        ErrorCode::InvalidParams
    );
    let error = a
        .call(
            m::SYNC_SET_REMOTE,
            json!({ "url": "https://sentinel-tok@example.test/repo.git" }),
        )
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidParams);
    assert!(!error.message.contains("sentinel-tok"));
    let st: SyncStatus = a.ok(m::SYNC_STATUS, json!(null));
    assert!(st.remote_url.is_none());
    // Simulate a legacy config written outside Latch. Status still redacts it.
    git(
        &a.vault_path(),
        &[
            "config",
            "remote.origin.url",
            "https://sentinel-tok@example.test/repo.git",
        ],
    );
    let st: SyncStatus = a.ok(m::SYNC_STATUS, json!(null));
    assert_eq!(
        st.remote_url.as_deref(),
        Some("https://***@example.test/repo.git")
    );
}

#[test]
fn unlock_timeout_must_be_bounded() {
    let a = TestAgent::new();
    a.init();
    a.agent.lock();
    for t in [0u64, 8 * 24 * 3600, u64::MAX] {
        assert_eq!(
            a.err(
                m::VAULT_UNLOCK,
                json!({ "passphrase": PASS, "identityFile": null, "timeoutSecs": t })
            ),
            ErrorCode::InvalidParams
        );
    }
    let st: VaultStatus = a.ok(
        m::VAULT_UNLOCK,
        json!({ "passphrase": PASS, "identityFile": null, "timeoutSecs": 60 }),
    );
    assert!(st.locks_in_secs.is_some_and(|s| s <= 60));
}

#[test]
fn start_pages_and_launch_urls_are_validated() {
    let a = TestAgent::new();
    a.init();
    let mut input = profile_input("x");
    input.browser.start_pages = vec!["--remote-debugging-port=9222".into()];
    assert_eq!(
        a.err(m::PROFILES_CREATE, serde_json::to_value(&input).unwrap()),
        ErrorCode::InvalidParams
    );
    let p = a.create_profile("y");
    let start = Instant::now();
    assert_eq!(
        a.err(
            m::BROWSER_LAUNCH,
            json!({ "profileId": p.id, "url": "--incognito" })
        ),
        ErrorCode::InvalidParams
    );
    assert!(start.elapsed() < Duration::from_secs(5));
}

#[test]
fn retired_native_client_kinds_are_rejected() {
    for client in ["native_host_chromium", "native_host_firefox"] {
        assert!(
            serde_json::from_value::<HelloParams>(json!({
                "client": client,
                "version": "test",
                "sessionToken": "legacy-token",
            }))
            .is_err()
        );
    }
}
