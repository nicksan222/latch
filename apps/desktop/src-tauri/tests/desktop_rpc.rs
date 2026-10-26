//! Desktop shell logic without a window: the protobuf command handler, the
//! tray model fed from a real agent, event effects and the socket handover.
#![cfg(feature = "desktop")]

use latch::agent::clipboard::{ClipboardBackend, MemoryClipboard};
use latch::rpc::{Agent, AgentOptions, ClientCtx, LocalClient};
use latch_desktop::desktop::events::{self, Effects};
use latch_desktop::desktop::rpc::{RpcErrorPayload, handle_rpc};
use latch_desktop::desktop::socket::{SocketOutcome, bind_or_take_over};
use latch_desktop::desktop::tray::{TrayAction, TrayInput, TrayItem, build_model};
use latch_rpc::{sdk::RpcSdk, v1::*};
use prost::Message;
use std::sync::{Arc, Once};
const PASS: &str = "correct horse battery staple";
struct TestAgent {
    agent: Arc<Agent>,
    dir: tempfile::TempDir,
    clip: Arc<MemoryClipboard>,
}
impl TestAgent {
    fn new() -> Self {
        static SETUP: Once = Once::new();
        SETUP.call_once(|| unsafe {
            std::env::set_var("LATCH_ALLOW_WEAK_KDF", "1");
            std::env::set_var("GIT_CONFIG_GLOBAL", "/dev/null");
            std::env::set_var("GIT_CONFIG_NOSYSTEM", "1");
            std::env::set_var(
                "LATCH_RUNTIME_DIR",
                std::env::temp_dir().join(format!("latch-desktop-test-{}", std::process::id())),
            );
        });
        let dir = tempfile::tempdir().unwrap();
        let clip = Arc::new(MemoryClipboard::new());
        let agent = Agent::new(AgentOptions {
            config_dir: Some(dir.path().join("config")),
            clipboard: Some(clip.clone()),
            browser_config: Some(latch::agent::browser::ManagerConfig {
                ram_dir: dir.path().join("ram"),
                data_dir: dir.path().join("data"),
                browser_override: Some(dir.path().join("missing-browser")),
                term_timeout: std::time::Duration::from_millis(50),
                archive_wait: std::time::Duration::from_secs(2),
            }),
            ..Default::default()
        });
        Self { agent, dir, clip }
    }
    fn client(&self) -> LocalClient {
        LocalClient::new(&self.agent, ClientCtx::desktop())
    }
    fn init(&self) {
        self.client()
            .vault_init(InitParams {
                path: self.dir.path().join("vault").display().to_string(),
                passphrase: PASS.into(),
                kdf: Some(KdfParams {
                    memory_kib: 1024,
                    iterations: 1,
                    parallelism: 1,
                }),
                git_name: Some("Alice".into()),
                git_email: Some("alice@example.com".into()),
                ..Default::default()
            })
            .unwrap();
    }
    fn create_profile(&self, name: &str) -> Profile {
        self.client().profiles_create(profile_input(name)).unwrap()
    }
    fn create_login(&self, profile_id: &str, name: &str, urls: &[&str], password: &str) -> Entry {
        self.client()
            .entries_create(EntryInput {
                profile_id: profile_id.into(),
                name: name.into(),
                favorite: true,
                payload: Some(EntryPayload {
                    data: Some(entry_payload::Data::Login(LoginData {
                        username: format!("{name}-user"),
                        password: Some(password.into()),
                        urls: urls.iter().map(|s| (*s).into()).collect(),
                        totp: Some(TotpConfig {
                            secret: Some("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ".into()),
                            algorithm: TotpAlgorithm::Sha1 as i32,
                            digits: 6,
                            period: 30,
                            issuer: None,
                        }),
                        ..Default::default()
                    })),
                }),
                ..Default::default()
            })
            .unwrap()
    }
}
impl Drop for TestAgent {
    fn drop(&mut self) {
        self.agent.shutdown();
    }
}
fn profile_input(name: &str) -> ProfileInput {
    ProfileInput {
        name: name.into(),
        color: "#2563eb".into(),
        icon: "briefcase".into(),
        browser: Some(BrowserSettings {
            autofill: AutofillScope::All as i32,
            ..Default::default()
        }),
        ..Default::default()
    }
}
fn request<Q: Message, R: Message + Default>(
    a: &TestAgent,
    method: &str,
    request: Q,
) -> Result<R, latch_rpc::errors::RpcError> {
    handle_rpc(&a.agent, method, &request.encode_to_vec())
        .map(|bytes| R::decode(bytes.as_slice()).unwrap())
}

fn actions(items: &[TrayItem]) -> Vec<TrayAction> {
    let mut out = vec![];
    for i in items {
        match i {
            TrayItem::Action { action, .. } => out.push(action.clone()),
            TrayItem::Submenu { items, .. } => out.extend(actions(items)),
            _ => {}
        }
    }
    out
}

fn tray_input(a: &TestAgent) -> TrayInput {
    let mut client = a.client();
    let mut input = TrayInput::from_status(&client.vault_status().unwrap());
    if input.initialized && !input.locked {
        input.profiles = client.profiles_list().unwrap().items;
        input.pinned = client.totp_pinned().unwrap().items;
    }
    input
}

#[test]
fn rpc_handler_results_and_errors() {
    let a = TestAgent::new();
    // Error payload: wire shape { code, message }.
    let e = request::<_, ProfilesListResponse>(&a, "ProfilesList", Empty {}).unwrap_err();
    assert_eq!(e.code, ErrorCode::NoVault);
    let v = serde_json::to_value(RpcErrorPayload::from(e)).unwrap();
    assert_eq!(v["code"], ErrorCode::NoVault as i32);
    assert!(v["message"].is_string());
    assert_eq!(v.as_object().unwrap().len(), 2);
    assert_eq!(
        handle_rpc(&a.agent, "bogus.method", &[]).unwrap_err().code,
        ErrorCode::InvalidParams
    );
    a.init();
    let st: VaultStatus = request(&a, "VaultStatus", Empty {}).unwrap();
    assert!(st.initialized && !st.locked);
    let p: Profile = request(&a, "ProfilesCreate", profile_input("acme")).unwrap();
    let entry = a.create_login(&p.id, "GitHub", &["https://github.com"], "pw-1");

    // clipboard.copy (the tray's TOTP action): value goes to the backend,
    // never into the RPC result.
    let r: ClipboardCopyResult = request(
        &a,
        "ClipboardCopy",
        ClipboardCopyParams {
            id: entry.id,
            field: "totp".into(),
            clear_after_secs: None,
        },
    )
    .unwrap();
    assert!(r.clear_after_secs > 0);
    let code = a.clip.get_text().expect("copied");
    assert_eq!(code.len(), 6);
    assert!(
        !r.encode_to_vec()
            .windows(code.len())
            .any(|bytes| bytes == code.as_bytes())
    );
    let _: VaultStatus = request(&a, "VaultLock", Empty {}).unwrap();
    assert_eq!(
        a.client().profiles_list().unwrap_err().code,
        ErrorCode::Locked
    );
    let e = request::<_, VaultStatus>(
        &a,
        "VaultUnlock",
        UnlockParams {
            passphrase: Some("zq-not-it-917".into()),
            ..Default::default()
        },
    )
    .unwrap_err();
    assert_eq!(e.code, ErrorCode::BadCredentials);
    assert!(!e.message.contains("zq-not-it-917"));
    let status: VaultStatus = request(
        &a,
        "VaultUnlock",
        UnlockParams {
            passphrase: Some(PASS.into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(!status.locked);
}

#[test]
fn tray_model_follows_agent_state_and_events() {
    let a = TestAgent::new();
    let mut client = a.client();
    let rx = client.watch_events().unwrap();
    assert_eq!(
        actions(&build_model(&tray_input(&a))),
        vec![TrayAction::Open, TrayAction::Quit]
    );

    a.init();
    let p = a.create_profile("acme");
    let entry = a.create_login(&p.id, "GitHub", &["https://github.com"], "pw-1");
    let acts = actions(&build_model(&tray_input(&a)));
    assert_eq!(
        acts,
        vec![
            TrayAction::Lock,
            TrayAction::OpenBrowser(p.id.clone()),
            TrayAction::CopyTotp(entry.id.clone()),
            TrayAction::Open,
            TrayAction::Quit,
        ]
    );

    a.client().vault_lock().unwrap();
    assert_eq!(
        actions(&build_model(&tray_input(&a))),
        vec![TrayAction::Unlock, TrayAction::Open, TrayAction::Quit]
    );

    // Every event the agent emitted along the way asks for a tray rebuild
    // when it changes what the tray shows.
    let mut kinds = vec![];
    while let Ok(Ok(ev)) = rx.recv_timeout(std::time::Duration::from_millis(250)) {
        kinds.push(ev.kind);
        let fx = events::effects(&ev);
        match AgentEventKind::try_from(ev.kind).unwrap() {
            AgentEventKind::Unlocked
            | AgentEventKind::Locked
            | AgentEventKind::ProfilesChanged
            | AgentEventKind::EntriesChanged => assert!(fx.rebuild_tray, "{:?}", ev.kind),
            AgentEventKind::SyncChanged => assert_eq!(fx, Effects::default()),
            _ => {}
        }
    }
    assert!(kinds.contains(&(AgentEventKind::Unlocked as i32)));
    assert!(kinds.contains(&(AgentEventKind::Locked as i32)));
}

#[test]
fn socket_bind_and_foreign_agent() {
    let a = TestAgent::new();
    match bind_or_take_over(&a.agent) {
        SocketOutcome::Bound(p) => assert!(p.exists()),
        o => panic!("expected bind, got {o:?}"),
    }
    // A second agent in the same runtime dir finds the lock held by a
    // process that is not a headless CLI agent (this test binary): it must
    // not kill it and runs without the socket.
    let b = TestAgent::new();
    match bind_or_take_over(&b.agent) {
        SocketOutcome::Unavailable(msg) => assert!(msg.contains("already running")),
        o => panic!("expected unavailable, got {o:?}"),
    }
    a.agent.shutdown();
    b.agent.shutdown();
}
