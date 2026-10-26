// Dispatcher end-to-end: lifecycle, every method's happy path, locked
// errors, auto-lock, clipboard, integrity surfacing.

#[path = "../../../../tests/agent_support/mod.rs"]
mod agent_support;

use std::time::Duration;

use agent_support::methods as m;
use agent_support::*;
use latch::agent::clipboard::ClipboardBackend;
use latch::core::format;
use latch::protocol::*;
use serde_json::{Value, json};

fn null() -> Value {
    Value::Null
}

#[test]
fn path_bound_and_totp_parse_procedures_work_while_locked() {
    let a = TestAgent::new();
    a.init();
    a.agent.lock();
    let root = a.vault_path().display().to_string();
    let located: TextResult = a.ok(m::VAULT_LOCATE, json!({"path": root}));
    assert_eq!(located.text, root);
    let report: DoctorReport = a.ok(m::DOCTOR_SCAN, json!({"path": root}));
    assert!(report.ok);
    let removed: HookResult = a.ok(
        m::HOOKS_UNINSTALL,
        json!({"path": root, "executable": null}),
    );
    assert!(removed.removed);
    let installed: HookResult = a.ok(m::HOOKS_INSTALL, json!({"path": root, "executable": null}));
    assert!(installed.path.unwrap().ends_with("pre-commit"));
    let parsed: TotpConfig = a.ok(m::TOTP_PARSE, json!({"text": "jbsw y3dp ehpk 3pxp"}));
    assert_eq!(parsed.secret.as_deref(), Some("JBSWY3DPEHPK3PXP"));
    assert_eq!(
        a.err(m::TOTP_PARSE, json!({"text": "not base32!"})),
        ErrorCode::InvalidParams
    );
    let non_vault = a.dir.path().join("not-vault");
    std::fs::create_dir(&non_vault).unwrap();
    assert_eq!(
        a.err(m::DOCTOR_SCAN, json!({"path": non_vault})),
        ErrorCode::NoVault
    );
}

fn id(id: &str) -> Value {
    json!({ "id": id })
}

fn drain(rx: &mut tokio::sync::broadcast::Receiver<AgentEvent>) -> Vec<AgentEvent> {
    let mut v = vec![];
    while let Ok(e) = rx.try_recv() {
        v.push(e);
    }
    v
}

#[test]
fn no_vault_configured() {
    let a = TestAgent::new();
    let st: VaultStatus = a.ok(m::VAULT_STATUS, null());
    assert!(!st.initialized);
    assert!(st.locked);
    assert_eq!(a.err(m::PROFILES_LIST, null()), ErrorCode::NoVault);
    assert_eq!(
        a.err(m::VAULT_UNLOCK, json!({"passphrase": PASS})),
        ErrorCode::NoVault
    );
    let l: Vec<KnownVault> = a.ok(m::VAULT_LIST, null());
    assert!(l.is_empty());
    // null params behave like {} for all-optional param structs.
    assert_eq!(a.err(m::ENTRIES_LIST, null()), ErrorCode::NoVault);
    assert_eq!(a.err(m::ENTRIES_TRASH, null()), ErrorCode::NoVault);
    assert_eq!(a.err("bogus.method", null()), ErrorCode::InvalidParams);
    // Malformed params never echo input.
    let e = a
        .call(m::PROFILES_GET, json!({"id": 42, "secretish": "hunter2"}))
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::InvalidParams);
    assert!(!e.message.contains("42") && !e.message.contains("hunter2"));
}

#[test]
fn init_lock_unlock_cycle() {
    let a = TestAgent::new();
    let mut rx = a.agent.subscribe();
    let r = a.init();
    assert!(r.status.initialized && !r.status.locked);
    assert!(r.status.locks_in_secs.is_some());
    assert!(r.recovery_kit.is_none());
    assert!(a.agent.is_unlocked());
    let ev = drain(&mut rx);
    assert!(ev.iter().any(|e| e.kind == AgentEventKind::Unlocked));

    let l: Vec<KnownVault> = a.ok(m::VAULT_LIST, null());
    assert_eq!(l.len(), 1);
    assert!(l[0].active);
    assert!(a.dir.path().join("config/vaults.json").exists());

    let p = a.create_profile("acme");
    let st: VaultStatus = a.ok(m::VAULT_LOCK, null());
    assert!(st.locked && st.initialized);
    assert_eq!(st.locks_in_secs, None);
    assert!(
        drain(&mut rx)
            .iter()
            .any(|e| e.kind == AgentEventKind::Locked)
    );

    // Locked errors.
    for (meth, params) in [
        (m::PROFILES_LIST, null()),
        (m::PROFILES_GET, id(&p.id)),
        (m::ENTRIES_LIST, json!({})),
        (m::ENTRIES_GET, json!({"id": p.id, "reveal": true})),
        (m::ENV_RESOLVE, json!({"profileId": p.id})),
        (m::TOTP_PINNED, null()),
        (m::CLIPBOARD_COPY, json!({"id": p.id, "field": "password"})),
        (m::SYNC_STATUS, null()),
        (m::RECIPIENTS_LIST, null()),
        (
            m::DOCTOR_RUN,
            json!({"fix": true, "rebuildManifest": false, "staged": false}),
        ),
    ] {
        assert_eq!(a.err(meth, params), ErrorCode::Locked, "{meth}");
    }
    // settings.get works locked, settings.update does not.
    let st: Settings = a.ok(m::SETTINGS_GET, null());
    assert_eq!(
        a.err(m::SETTINGS_UPDATE, serde_json::to_value(&st).unwrap()),
        ErrorCode::Locked
    );
    let _: Vec<DetectedBrowser> = a.ok(m::BROWSER_DETECT, null());
    let _: Vec<BrowserSession> = a.ok(m::BROWSER_LIST, null());
    // Locked doctor without fix = plaintext scan (no keys needed).
    let r: DoctorReport = a.ok(
        m::DOCTOR_RUN,
        json!({"fix": false, "rebuildManifest": false, "staged": false}),
    );
    assert!(r.ok, "{:?}", r.issues);

    // Wrong passphrase.
    let e = a
        .call(m::VAULT_UNLOCK, json!({"passphrase": "wrong passphrase!!"}))
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::BadCredentials);
    assert!(!e.message.contains("wrong passphrase!!"));
    assert_eq!(
        a.err(
            m::VAULT_UNLOCK,
            json!({"passphrase": PASS, "identityFile": "/x"})
        ),
        ErrorCode::InvalidParams
    );

    let st = a.unlock();
    assert!(!st.locked);
    let list: Vec<ProfileSummary> = a.ok(m::PROFILES_LIST, null());
    assert_eq!(list.len(), 1);
    assert!(!list[0].browser_running);
    // Unlock while unlocked is a no-op.
    let st = a.unlock();
    assert!(!st.locked);

    // A fresh agent over the same config dir finds the active vault.
    drop(rx);
}

#[test]
fn every_method_happy_path() {
    let a = TestAgent::new();
    a.init();
    let hello: HelloResult = a.ok(
        m::HELLO,
        json!({"client": "desktop", "version": "1", "sessionToken": null}),
    );
    assert_eq!(hello.protocol, PROTOCOL_VERSION);

    // ----- profiles
    let p = a.create_profile("acme");
    let p2 = a.create_profile("other");
    let got: Profile = a.ok(m::PROFILES_GET, id(&p.id));
    assert_eq!(got.name, "acme");
    let mut input = profile_input("acme-renamed");
    input.pinned = true;
    let up: Profile = a.ok(m::PROFILES_UPDATE, json!({"id": p.id, "input": input}));
    assert_eq!(up.name, "acme-renamed");
    let list: Vec<ProfileSummary> = a.ok(m::PROFILES_LIST, null());
    assert_eq!(list[0].id, p.id, "pinned first");

    // ----- entries
    let login = a.create_login(&p.id, "GitHub", &["https://github.com"], "gh-pass");
    assert!(login.redacted_fields.contains(&"password".to_string()));
    let env: Entry = a.ok(
        m::ENTRIES_CREATE,
        serde_json::to_value(EntryInput {
            profile_id: p.id.clone(),
            name: "app env".into(),
            tags: vec!["t".into()],
            notes: String::new(),
            favorite: false,
            payload: EntryPayload::Env(EnvData {
                vars: vec![
                    EnvVar {
                        key: "A".into(),
                        value: Some("1".into()),
                        hidden: true,
                        comment: None,
                    },
                    EnvVar {
                        key: "B".into(),
                        value: Some("two".into()),
                        hidden: false,
                        comment: None,
                    },
                ],
            }),
        })
        .unwrap(),
    );
    let list: Vec<EntrySummary> = a.ok(m::ENTRIES_LIST, json!({"profileId": p.id}));
    assert_eq!(list.len(), 2);
    let list: Vec<EntrySummary> = a.ok(m::ENTRIES_LIST, json!({"query": "git"}));
    assert_eq!(list.len(), 1);
    let e: Entry = a.ok(m::ENTRIES_GET, json!({"id": login.id, "reveal": false}));
    match &e.payload {
        EntryPayload::Login(l) => assert_eq!(l.password, None),
        _ => panic!(),
    }
    let e: Entry = a.ok(m::ENTRIES_GET, json!({"id": login.id, "reveal": true}));
    match &e.payload {
        EntryPayload::Login(l) => assert_eq!(l.password.as_deref(), Some("gh-pass")),
        _ => panic!(),
    }
    let r: RevealResult = a.ok(
        m::ENTRIES_REVEAL,
        json!({"id": login.id, "field": "password"}),
    );
    assert_eq!(r.value, "gh-pass");
    let mut inp = EntryInput {
        profile_id: p.id.clone(),
        name: "GitHub".into(),
        tags: vec![],
        notes: "updated".into(),
        favorite: true,
        payload: e.payload.clone(),
    };
    if let EntryPayload::Login(l) = &mut inp.payload {
        l.password = Some("gh-pass-2".into());
    }
    let _: Entry = a.ok(m::ENTRIES_UPDATE, json!({"id": login.id, "input": inp}));
    let h: Vec<HistoryItem> = a.ok(m::ENTRIES_HISTORY, id(&login.id));
    assert_eq!(h.len(), 2);
    let old_commit = h[1].commit.clone();
    let restored: Entry = a.ok(
        m::ENTRIES_RESTORE,
        json!({"id": login.id, "commit": old_commit}),
    );
    assert_eq!(restored.id, login.id);
    let r: RevealResult = a.ok(
        m::ENTRIES_REVEAL,
        json!({"id": login.id, "field": "password"}),
    );
    assert_eq!(r.value, "gh-pass");

    // ----- env / totp
    let er: EnvResolveResult = a.ok(m::ENV_RESOLVE, json!({"profileId": p.id, "entryIds": null}));
    assert_eq!(er.vars.len(), 2);
    let t: TextResult = a.ok(
        m::ENV_FORMAT,
        json!({"profileId": p.id, "entryIds": [env.id], "format": "export"}),
    );
    assert!(t.text.contains("export A="));
    let c: ClipboardCopyResult = a.ok(
        m::ENV_COPY,
        json!({"profileId": p.id, "entryIds": null, "format": "dotenv"}),
    );
    assert_eq!(c.clear_after_secs, 30);
    let clip = a.clip.get_text().unwrap();
    assert!(
        clip.contains("A=") && clip.contains("two"),
        "dotenv on clipboard"
    );
    let parsed: Vec<EnvVar> = a.ok(m::ENV_PARSE, json!({"text": "X=1\nY=\"two\"\n"}));
    assert_eq!(parsed.len(), 2);
    let code: TotpCode = a.ok(m::TOTP_GET, id(&login.id));
    assert_eq!(code.code.len(), 6);
    let pinned: Vec<PinnedTotp> = a.ok(m::TOTP_PINNED, null());
    assert_eq!(pinned.len(), 1);

    // ----- clipboard
    let c: ClipboardCopyResult = a.ok(
        m::CLIPBOARD_COPY,
        json!({"id": login.id, "field": "username"}),
    );
    assert_eq!(c.clear_after_secs, 30);
    assert_eq!(a.clip.get_text().as_deref(), Some("GitHub-user"));
    let _: ClipboardCopyResult = a.ok(m::CLIPBOARD_COPY, json!({"id": login.id, "field": "totp"}));
    assert_eq!(a.clip.get_text().unwrap().len(), 6);

    // ----- move, delete, trash
    let moved: Entry = a.ok(m::ENTRIES_MOVE, json!({"id": env.id, "profileId": p2.id}));
    assert_eq!(moved.profile_id, p2.id);
    let _: Value = a.ok(m::ENTRIES_DELETE, id(&env.id));
    let trash: Vec<TrashItem> = a.ok(m::ENTRIES_TRASH, json!({"profileId": null}));
    assert_eq!(trash.len(), 1);
    assert_eq!(trash[0].id, env.id);

    // ----- browser (no browser is launched here)
    let _: Vec<BrowserSession> = a.ok(m::BROWSER_LIST, null());
    let _: Vec<DetectedBrowser> = a.ok(m::BROWSER_DETECT, null());
    let _: SessionArchiveInfo = a.ok(m::BROWSER_SESSION_INFO, id(&p.id));
    let _: Value = a.ok(m::BROWSER_CLOSE, id(&p.id));

    // ----- recipients
    let rs: Vec<RecipientInfo> = a.ok(m::RECIPIENTS_LIST, null());
    assert_eq!(rs.len(), 1);
    let key = age::x25519::Identity::generate().to_public().to_string();
    let rs: Vec<RecipientInfo> = a.ok(m::RECIPIENTS_ADD, json!({"label": "bob", "publicKey": key}));
    assert_eq!(rs.len(), 2);
    let rs: Vec<RecipientInfo> = a.ok(m::RECIPIENTS_REMOVE, json!({"label": "bob"}));
    assert_eq!(rs.len(), 1);
    let _: Vec<RecipientInfo> = a.ok(m::RECIPIENTS_ROTATE, null());
    let r: RevealResult = a.ok(
        m::ENTRIES_REVEAL,
        json!({"id": login.id, "field": "password"}),
    );
    assert_eq!(r.value, "gh-pass");

    // ----- settings (machine + vault-scoped)
    let mut s: Settings = a.ok(m::SETTINGS_GET, null());
    assert_eq!(s.auto_lock_secs, 900);
    s.clipboard_clear_secs = 12;
    s.names_in_commits = true;
    let s2: Settings = a.ok(m::SETTINGS_UPDATE, serde_json::to_value(&s).unwrap());
    assert_eq!(s2.clipboard_clear_secs, 12);
    assert!(s2.names_in_commits);
    let cfg = std::fs::read_to_string(a.vault_path().join(format::CONFIG_FILE)).unwrap();
    assert!(cfg.contains("names_in_commits = true"));
    let stored = std::fs::read_to_string(a.dir.path().join("config/settings.json")).unwrap();
    assert!(stored.contains("\"clipboardClearSecs\": 12"));

    // ----- doctor
    let r: DoctorReport = a.ok(
        m::DOCTOR_RUN,
        json!({"fix": false, "rebuildManifest": false, "staged": false}),
    );
    assert!(r.ok, "{:?}", r.issues);
    let r: DoctorReport = a.ok(
        m::DOCTOR_RUN,
        json!({"fix": false, "rebuildManifest": false, "staged": true}),
    );
    assert!(r.ok, "{:?}", r.issues);

    // ----- sync with a bare remote, then clone it as a second vault
    let remote = tmpdir();
    git(remote.path(), &["init", "--bare", "--quiet", "-b", "main"]);
    let url = remote.path().display().to_string();
    let ss: SyncStatus = a.ok(m::SYNC_SET_REMOTE, json!({"url": url}));
    assert!(ss.has_remote);
    let sr: SyncResult = a.ok(m::SYNC_RUN, json!({"mode": "both"}));
    assert!(sr.conflicts.is_empty());
    let ss: SyncStatus = a.ok(m::SYNC_STATUS, null());
    assert_eq!(ss.ahead, 0);
    assert!(a.call(m::SYNC_CONFLICT, id(&login.id)).is_err());

    // ----- import/export (implemented by another module; must not be
    // denied or panic)
    let out = a.dir.path().join("export.json");
    match a.call(m::EXPORT_RUN, json!({"format": "json", "path": out.display().to_string(), "profileId": null, "passphrase": null})) {
        Ok(v) => {
            let r: ExportResult = serde_json::from_value(v).unwrap();
            assert!(r.entries >= 1);
        }
        Err(e) => assert!(matches!(e.code, ErrorCode::Internal | ErrorCode::InvalidParams), "{e:?}"),
    }
    let envfile = a.dir.path().join("in.env");
    std::fs::write(&envfile, "IMPORTED=1\n").unwrap();
    let ip = json!({"format": "dotenv", "path": envfile.display().to_string(), "profileId": p2.id, "skipDuplicates": true, "passphrase": null});
    if let Err(e) = a.call(m::IMPORT_PREVIEW, ip.clone()) {
        assert!(
            matches!(e.code, ErrorCode::Internal | ErrorCode::InvalidParams),
            "{e:?}"
        );
    }
    if let Err(e) = a.call(m::IMPORT_APPLY, ip) {
        assert!(
            matches!(e.code, ErrorCode::Internal | ErrorCode::InvalidParams),
            "{e:?}"
        );
    }

    // ----- change passphrase
    let _: Value = a.ok(
        m::VAULT_CHANGE_PASSPHRASE,
        json!({"current": PASS, "newPassphrase": "another long passphrase"}),
    );
    let _: VaultStatus = a.ok(m::VAULT_LOCK, null());
    assert_eq!(
        a.err(m::VAULT_UNLOCK, json!({"passphrase": PASS})),
        ErrorCode::BadCredentials
    );
    let _: VaultStatus = a.ok(
        m::VAULT_UNLOCK,
        json!({"passphrase": "another long passphrase"}),
    );

    // ----- profiles.delete
    let _: Value = a.ok(m::PROFILES_DELETE, id(&p2.id));
    let list: Vec<ProfileSummary> = a.ok(m::PROFILES_LIST, null());
    assert_eq!(list.len(), 1);

    // ----- vault.clone + vault.open switch the active vault
    let cloned = a.dir.path().join("clone");
    let st: VaultStatus = a.ok(
        m::VAULT_CLONE,
        json!({"url": url, "path": cloned.display().to_string()}),
    );
    assert!(st.locked && st.initialized);
    assert!(!a.agent.is_unlocked(), "switching vaults locks");
    let l: Vec<KnownVault> = a.ok(m::VAULT_LIST, null());
    assert_eq!(l.len(), 2);
    assert!(l.iter().any(|k| k.active && k.path.ends_with("clone")));
    let st: VaultStatus = a.ok(
        m::VAULT_OPEN,
        json!({"path": a.vault_path().display().to_string()}),
    );
    assert!(st.locked);
    let l: Vec<KnownVault> = a.ok(m::VAULT_LIST, null());
    assert!(l.iter().any(|k| k.active && !k.path.ends_with("clone")));
    assert_eq!(
        a.err(m::VAULT_OPEN, json!({"path": "/definitely/not/here"})),
        ErrorCode::NoVault
    );
}

#[test]
fn benchmark_kdf() {
    let a = TestAgent::new();
    let b: KdfBenchmark = a.ok(m::VAULT_BENCHMARK_KDF, json!({"targetMs": 50}));
    assert!(b.params.memory_kib >= 64 * 1024);
}

#[test]
fn idle_auto_lock() {
    let a = TestAgent::new();
    a.init();
    let mut s: Settings = a.ok(m::SETTINGS_GET, null());
    s.auto_lock_secs = 1;
    let _: Settings = a.ok(m::SETTINGS_UPDATE, serde_json::to_value(&s).unwrap());
    let mut rx = a.agent.subscribe();
    let st: VaultStatus = a.ok(m::VAULT_STATUS, null());
    assert!(st.locks_in_secs.unwrap() <= 1);
    assert!(wait_until(Duration::from_secs(5), || !a
        .agent
        .is_unlocked()));
    let ev = drain(&mut rx);
    let locked = ev
        .iter()
        .find(|e| e.kind == AgentEventKind::Locked)
        .expect("Locked event");
    assert_eq!(locked.message.as_deref(), Some("idle timeout"));
    assert_eq!(a.err(m::PROFILES_LIST, null()), ErrorCode::Locked);

    // Per-session timeout override; status calls do not reset the timer
    // but other requests do.
    let _: VaultStatus = a.ok(
        m::VAULT_UNLOCK,
        json!({"passphrase": PASS, "timeoutSecs": 2}),
    );
    let st: VaultStatus = a.ok(m::VAULT_STATUS, null());
    assert_eq!(st.locks_in_secs, Some(2));
    drain(&mut rx);
    for _ in 0..6 {
        std::thread::sleep(Duration::from_millis(500));
        let _: Vec<ProfileSummary> = a.ok(m::PROFILES_LIST, null());
    }
    assert!(a.agent.is_unlocked(), "activity keeps it unlocked");
    // Passive calls do not reset the timer.
    let end = std::time::Instant::now() + Duration::from_secs(5);
    while a.agent.is_unlocked() && std::time::Instant::now() < end {
        let _: VaultStatus = a.ok(m::VAULT_STATUS, null());
        let _: Settings = a.ok(m::SETTINGS_GET, null());
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(!a.agent.is_unlocked());
    let ev = drain(&mut rx);
    let w = ev
        .iter()
        .position(|e| e.kind == AgentEventKind::LockWarning)
        .expect("LockWarning");
    let l = ev
        .iter()
        .position(|e| e.kind == AgentEventKind::Locked)
        .expect("Locked");
    assert!(w < l, "warning precedes lock");

    // autoLockSecs = 0 disables the idle lock.
    s.auto_lock_secs = 0;
    a.unlock();
    let _: Settings = a.ok(m::SETTINGS_UPDATE, serde_json::to_value(&s).unwrap());
    let st: VaultStatus = a.ok(m::VAULT_STATUS, null());
    assert_eq!(st.locks_in_secs, None);
    std::thread::sleep(Duration::from_millis(1300));
    assert!(a.agent.is_unlocked());
}

#[test]
fn screen_lock_respects_setting() {
    let a = TestAgent::new();
    a.init();
    let mut s: Settings = a.ok(m::SETTINGS_GET, null());
    s.lock_on_screen_lock = false;
    let _: Settings = a.ok(m::SETTINGS_UPDATE, serde_json::to_value(&s).unwrap());
    a.agent.notify_screen_locked();
    assert!(a.agent.is_unlocked());
    s.lock_on_screen_lock = true;
    let _: Settings = a.ok(m::SETTINGS_UPDATE, serde_json::to_value(&s).unwrap());
    a.agent.notify_screen_locked();
    assert!(!a.agent.is_unlocked());
}

#[test]
fn clipboard_clears_only_if_unchanged() {
    let a = TestAgent::new();
    a.init();
    let p = a.create_profile("acme");
    let e = a.create_login(&p.id, "site", &["https://site.test"], "pw-1");

    let r: ClipboardCopyResult = a.ok(
        m::CLIPBOARD_COPY,
        json!({"id": e.id, "field": "password", "clearAfterSecs": 1}),
    );
    assert_eq!(r.clear_after_secs, 1);
    assert_eq!(a.clip.get_text().as_deref(), Some("pw-1"));
    assert!(wait_until(Duration::from_secs(4), || a
        .clip
        .get_text()
        .is_none()));

    // User copies something else meanwhile: it must survive.
    let _: ClipboardCopyResult = a.ok(
        m::CLIPBOARD_COPY,
        json!({"id": e.id, "field": "password", "clearAfterSecs": 1}),
    );
    a.clip.set_text("user content").unwrap();
    std::thread::sleep(Duration::from_millis(1600));
    assert_eq!(a.clip.get_text().as_deref(), Some("user content"));

    // Shutdown flushes pending clears.
    let _: ClipboardCopyResult = a.ok(m::CLIPBOARD_COPY, json!({"id": e.id, "field": "password"}));
    assert_eq!(a.clip.get_text().as_deref(), Some("pw-1"));
    a.agent.shutdown();
    assert_eq!(a.clip.get_text(), None);

    // Unknown field.
    let b = TestAgent::new();
    b.init();
    let p = b.create_profile("x");
    let e = b.create_login(&p.id, "s", &[], "pw");
    assert_eq!(
        b.err(m::CLIPBOARD_COPY, json!({"id": e.id, "field": "nope"})),
        ErrorCode::NotFound
    );
}

#[test]
fn integrity_problems_are_surfaced() {
    let a = TestAgent::new();
    a.init();
    let p = a.create_profile("acme");
    let e = a.create_login(&p.id, "site", &["https://site.test"], "pw-1");
    a.agent.lock();
    let f = a.vault_path().join(format::entry_file(&p.id, &e.id));
    let mut bytes = std::fs::read(&f).unwrap();
    let n = bytes.len();
    bytes[n - 5] ^= 0x55;
    std::fs::write(&f, &bytes).unwrap();

    let mut rx = a.agent.subscribe();
    let st = a.unlock();
    assert!(!st.locked, "unlock still succeeds");
    let ev = drain(&mut rx);
    let warn = ev
        .iter()
        .find(|e| e.kind == AgentEventKind::VaultChanged && e.message.is_some())
        .expect("integrity event");
    assert!(warn.message.as_ref().unwrap().contains("integrity"));
    assert!(!a.agent.integrity_problems().is_empty());
    assert_eq!(a.err(m::PROFILES_LIST, null()), ErrorCode::Integrity);
    assert_eq!(
        a.err(m::ENTRIES_GET, json!({"id": e.id, "reveal": false})),
        ErrorCode::Integrity
    );
    assert_eq!(
        a.err(
            m::PROFILES_CREATE,
            serde_json::to_value(profile_input("x")).unwrap()
        ),
        ErrorCode::Integrity
    );

    let r: DoctorReport = a.ok(
        m::DOCTOR_RUN,
        json!({"fix": false, "rebuildManifest": false, "staged": false}),
    );
    assert!(!r.ok);
    let r: DoctorReport = a.ok(
        m::DOCTOR_RUN,
        json!({"fix": true, "rebuildManifest": false, "staged": false}),
    );
    assert!(r.ok, "{:?}", r.issues);
    assert!(a.agent.integrity_problems().is_empty());
    let ev = drain(&mut rx);
    assert!(
        ev.iter()
            .any(|e| e.message.as_deref() == Some("integrity restored"))
    );
    let rv: RevealResult = a.ok(m::ENTRIES_REVEAL, json!({"id": e.id, "field": "password"}));
    assert_eq!(rv.value, "pw-1");
}

#[test]
fn errors_and_events_never_contain_secrets() {
    let a = TestAgent::new();
    a.init();
    let mut rx = a.agent.subscribe();
    let p = a.create_profile("acme");
    let e = a.create_login(&p.id, "site", &["https://site.test"], "super-secret-pw");
    let _: ClipboardCopyResult = a.ok(m::CLIPBOARD_COPY, json!({"id": e.id, "field": "password"}));
    let _ = a.call(
        m::ENTRIES_REVEAL,
        json!({"id": e.id, "field": "custom.nope"}),
    );
    for ev in drain(&mut rx) {
        let s = serde_json::to_string(&ev).unwrap();
        assert!(!s.contains("super-secret-pw"), "{s}");
    }
    let dbg = format!("{:?}", a.agent);
    assert!(!dbg.contains("super-secret-pw"));
}

#[test]
fn clipboard_duration_override_preserves_full_wire_range_and_caps_at_one_day() {
    let a = TestAgent::new();
    a.init();
    let profile = a.create_profile("clipboard-duration");
    let entry = a.create_login(&profile.id, "service", &[], "private-clipboard-value");
    let result: ClipboardCopyResult = a.ok(
        m::CLIPBOARD_COPY,
        json!({ "id": entry.id, "field": "password", "clearAfterSecs": u64::MAX }),
    );
    assert_eq!(result.clear_after_secs, 24 * 3600);
    assert_eq!(
        a.clip.get_text().as_deref(),
        Some("private-clipboard-value")
    );
    assert!(
        !serde_json::to_string(&result)
            .unwrap()
            .contains("private-clipboard-value")
    );
}
