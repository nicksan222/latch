// Typed dispatch contract checks shared by the direct and fixture-driven tests.

#[path = "../../../tests/agent_support/mod.rs"]
mod agent_support;

#[cfg(target_os = "linux")]
use std::collections::HashSet;
use std::sync::Once;
use std::time::Duration;

use agent_support::{TestAgent, methods};
use latch::agent::ClientCtx;
use latch::agent::browser::ManagerConfig;
use latch::protocol::*;
use latch_rpc::{sdk::Call, v1};
use serde_json::{Value, json};

fn isolated_agent() -> TestAgent {
    static SETUP: Once = Once::new();
    SETUP.call_once(|| {
        agent_support::setup();
        // browser.detect uses process configuration rather than ManagerConfig.
        #[cfg(target_os = "linux")]
        unsafe {
            let absent =
                std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("typed-dispatch-no-browser");
            std::env::set_var("PATH", &absent);
        }
    });
    TestAgent::with(|opts| {
        let root = opts.config_dir.as_ref().unwrap().parent().unwrap();
        opts.browser_config = Some(ManagerConfig {
            ram_dir: root.join("ram"),
            data_dir: root.join("data"),
            browser_override: None,
            term_timeout: Duration::from_millis(50),
            archive_wait: Duration::from_millis(50),
        });
        opts.watch_screen_lock = false;
        opts.headless_idle_exit = None;
        opts.browser_lock_grace = Duration::ZERO;
    })
}

#[cfg(target_os = "linux")]
#[test]
fn test_adapter_covers_every_unary_sdk_method() {
    let mut names = HashSet::new();
    assert!(!methods::ALL.is_empty());
    for &method in methods::ALL {
        assert!(!method.trim().is_empty(), "empty RPC method name");
        assert!(names.insert(method), "duplicate RPC method: {method}");
        // Probe registration without executing effects: malformed parameters
        // must be rejected before Agent receives a typed Call.
        if let Err(error) = agent_support::test_call(method, json!([])) {
            assert_eq!(error.code, ErrorCode::InvalidParams, "{method}");
            assert!(
                !error.message.contains("unknown method"),
                "declared method is missing from the typed fixture adapter: {method}"
            );
            assert!(!error.message.contains("hunter2"));
        }
    }
}

#[test]
fn selected_typed_results_and_no_argument_unit_conventions_match() {
    let a = isolated_agent();
    let hello: HelloResult = a.ok(
        methods::HELLO,
        serde_json::to_value(HelloParams {
            client: ClientKind::Desktop,
            version: "contract-test".into(),
        })
        .unwrap(),
    );
    assert_eq!(hello.protocol, PROTOCOL_VERSION);
    assert_eq!(hello.agent_version, env!("CARGO_PKG_VERSION"));

    for params in [Value::Null, json!({})] {
        let status: VaultStatus = a.ok(methods::VAULT_STATUS, params.clone());
        assert!(status.locked && !status.initialized);
        assert_eq!(status.locks_in_secs, None);
        let vaults: Vec<KnownVault> = a.ok(methods::VAULT_LIST, params.clone());
        assert!(vaults.is_empty());
        let sessions: Vec<BrowserSession> = a.ok(methods::BROWSER_LIST, params.clone());
        assert!(sessions.is_empty());
        let settings: Settings = a.ok(methods::SETTINGS_GET, params.clone());
        assert_eq!(settings.auto_lock_secs, 900);
        let locked: VaultStatus = a.ok(methods::VAULT_LOCK, params);
        assert!(locked.locked && !locked.initialized);
    }

    // A direct typed call confirms that a unit protobuf reply remains a unit.
    let reply = a
        .agent
        .execute(
            &ClientCtx::desktop(),
            Call::BrowserClose(v1::IdParams {
                id: "no-running-session".into(),
            }),
        )
        .unwrap();
    assert!(matches!(
        reply,
        latch_rpc::sdk::Reply::BrowserClose(v1::Empty {})
    ));
}

#[test]
fn unknown_and_malformed_fixture_calls_fail_without_echoing_payloads() {
    let unknown = "protocol.contract.unknown";
    let error =
        agent_support::test_call(unknown, json!({"passphrase": "payload-must-not-be-echoed"}))
            .expect_err("unknown test method must fail before typed dispatch");
    assert_eq!(error.code, ErrorCode::InvalidParams);
    assert!(error.message.contains(unknown));
    assert!(!error.message.contains("payload-must-not-be-echoed"));

    let malformed = agent_support::test_call(
        methods::PROFILES_GET,
        json!({"id": ["payload-must-not-be-echoed"]}),
    )
    .expect_err("malformed owned input must fail");
    assert_eq!(malformed.code, ErrorCode::InvalidParams);
    assert_eq!(
        malformed.message,
        "invalid parameters: profiles.get: malformed parameters"
    );
    assert!(!malformed.message.contains("payload-must-not-be-echoed"));
}
