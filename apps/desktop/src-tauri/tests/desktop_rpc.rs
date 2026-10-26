//! Desktop shell logic without a window: the `rpc` command handler, the
//! tray model fed from a real agent, event effects and the socket handover.
#![cfg(feature = "desktop")]

#[path = "../../../../packages/core/tests/agent_support/mod.rs"]
mod agent_support;

use agent_support::*;
use latch::agent::clipboard::ClipboardBackend;
use latch::protocol::methods as m;
use latch::protocol::*;
use latch_desktop::desktop::events::{self, Effects};
use latch_desktop::desktop::rpc::handle_rpc;
use latch_desktop::desktop::socket::{SocketOutcome, bind_or_take_over};
use latch_desktop::desktop::tray::{TrayAction, TrayInput, TrayItem, build_model};
use serde_json::{Value, json};

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
    let mut input = TrayInput::from_status(&a.agent.status());
    if input.initialized && !input.locked {
        input.profiles =
            serde_json::from_value(handle_rpc(&a.agent, m::PROFILES_LIST, Value::Null).unwrap())
                .unwrap();
        input.pinned =
            serde_json::from_value(handle_rpc(&a.agent, m::TOTP_PINNED, Value::Null).unwrap())
                .unwrap();
    }
    input
}

#[test]
fn rpc_handler_results_and_errors() {
    let a = TestAgent::new();
    // Error payload: wire shape { code, message }.
    let e = handle_rpc(&a.agent, m::PROFILES_LIST, Value::Null).unwrap_err();
    assert_eq!(e.code, ErrorCode::NoVault);
    let v = serde_json::to_value(&e).unwrap();
    assert_eq!(v["code"], "no_vault");
    assert!(v["message"].is_string());
    assert_eq!(v.as_object().unwrap().len(), 2);
    assert_eq!(
        handle_rpc(&a.agent, "bogus.method", Value::Null)
            .unwrap_err()
            .code,
        ErrorCode::InvalidParams
    );

    // Full flow through the handler: init → profile → entry → lock/unlock.
    a.init();
    let st: VaultStatus =
        serde_json::from_value(handle_rpc(&a.agent, m::VAULT_STATUS, Value::Null).unwrap())
            .unwrap();
    assert!(st.initialized && !st.locked);
    let p: Profile = serde_json::from_value(
        handle_rpc(
            &a.agent,
            m::PROFILES_CREATE,
            serde_json::to_value(profile_input("acme")).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let entry = a.create_login(&p.id, "GitHub", &["https://github.com"], "pw-1");

    // clipboard.copy (the tray's TOTP action): value goes to the backend,
    // never into the RPC result.
    let r = handle_rpc(
        &a.agent,
        m::CLIPBOARD_COPY,
        json!({ "id": entry.id, "field": "totp" }),
    )
    .unwrap();
    assert!(r.get("clearAfterSecs").is_some());
    let code = a.clip.get_text().expect("copied");
    assert_eq!(code.len(), 6);
    assert!(!r.to_string().contains(&code));

    handle_rpc(&a.agent, m::VAULT_LOCK, Value::Null).unwrap();
    assert_eq!(
        handle_rpc(&a.agent, m::PROFILES_LIST, Value::Null)
            .unwrap_err()
            .code,
        ErrorCode::Locked
    );
    let e = handle_rpc(
        &a.agent,
        m::VAULT_UNLOCK,
        json!({ "passphrase": "zq-not-it-917" }),
    )
    .unwrap_err();
    assert_eq!(e.code, ErrorCode::BadCredentials);
    assert!(!e.message.contains("zq-not-it-917"));
    handle_rpc(&a.agent, m::VAULT_UNLOCK, json!({ "passphrase": PASS })).unwrap();
    assert!(a.agent.is_unlocked());
}

#[test]
fn tray_model_follows_agent_state_and_events() {
    let a = TestAgent::new();
    let mut rx = a.agent.subscribe();
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

    a.agent.lock();
    assert_eq!(
        actions(&build_model(&tray_input(&a))),
        vec![TrayAction::Unlock, TrayAction::Open, TrayAction::Quit]
    );

    // Every event the agent emitted along the way asks for a tray rebuild
    // when it changes what the tray shows.
    let mut kinds = vec![];
    while let Ok(ev) = rx.try_recv() {
        kinds.push(ev.kind);
        let fx = events::effects(&ev);
        match ev.kind {
            AgentEventKind::Unlocked
            | AgentEventKind::Locked
            | AgentEventKind::ProfilesChanged
            | AgentEventKind::EntriesChanged => assert!(fx.rebuild_tray, "{:?}", ev.kind),
            AgentEventKind::SyncChanged => assert_eq!(fx, Effects::default()),
            _ => {}
        }
    }
    assert!(kinds.contains(&AgentEventKind::Unlocked));
    assert!(kinds.contains(&AgentEventKind::Locked));
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
