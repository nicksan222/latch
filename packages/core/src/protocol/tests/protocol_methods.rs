// Contract checks between the RPC schema and the independently maintained dispatcher.

#[path = "../../../tests/agent_support/mod.rs"]
mod agent_support;

#[cfg(target_os = "linux")]
use std::collections::HashSet;
use std::sync::Once;
use std::time::Duration;

use agent_support::TestAgent;
use latch::agent::ClientCtx;
use latch::agent::browser::ManagerConfig;
use latch::protocol::methods as m;
use latch::protocol::*;
use serde_json::{Value, json};

fn isolated_agent() -> TestAgent {
    static SETUP: Once = Once::new();
    SETUP.call_once(|| {
        agent_support::setup();
        // browser.detect uses process configuration rather than ManagerConfig.
        // On Linux its only search locations are PATH and LATCH_BROWSER. All
        // tests in this binary pass this Once before creating any agent threads.
        #[cfg(target_os = "linux")]
        unsafe {
            let absent = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
                .join("protocol-methods-no-browser");
            std::env::set_var("PATH", &absent);
            std::env::set_var("LATCH_BROWSER", absent.join("missing-browser"));
        }
    });
    TestAgent::with(|opts| {
        let root = opts.config_dir.as_ref().unwrap().parent().unwrap();
        opts.browser_config = Some(ManagerConfig {
            ram_dir: root.join("ram"),
            data_dir: root.join("data"),
            browser_override: None,
            path_var: Some(root.join("empty-path").into_os_string()),
            term_timeout: Duration::from_millis(50),
            archive_wait: Duration::from_millis(50),
        });
        opts.watch_screen_lock = false;
        opts.headless_idle_exit = None;
        opts.browser_lock_grace = Duration::ZERO;
    })
}

// Other platforms also search fixed install locations; the current static
// browser.detect route cannot isolate those through AgentOptions.
#[cfg(target_os = "linux")]
#[test]
fn every_declared_method_reaches_a_route_without_a_vault() {
    let a = isolated_agent();
    let mut names = HashSet::new();
    assert!(!m::ALL.is_empty(), "RPC schema must declare methods");
    for &method in m::ALL {
        assert!(!method.trim().is_empty(), "empty RPC method name");
        assert!(names.insert(method), "duplicate RPC method: {method}");
        // Required fields are absent or the whole parameter shape is invalid.
        // No valid filesystem, network, plugin, or browser launch input is sent.
        for params in [json!({}), json!([])] {
            if let Err(error) = a.call(method, params) {
                assert!(
                    !error.message.contains("unknown method"),
                    "declared method has no dispatcher route: {method}"
                );
                assert!(
                    matches!(error.code, ErrorCode::InvalidParams | ErrorCode::NoVault),
                    "unexpected error code for {method}"
                );
            }
        }
    }
    let status: VaultStatus = a.ok(m::VAULT_STATUS, Value::Null);
    assert!(status.locked && !status.initialized);
    let browsers: Vec<DetectedBrowser> = a.ok(m::BROWSER_DETECT, Value::Null);
    assert!(
        browsers.is_empty(),
        "host browser detection was not isolated"
    );
    assert!(
        !a.vault_path().exists(),
        "routing probes must not create a vault"
    );
}

#[test]
fn selected_wire_results_and_no_argument_unit_conventions_match_dispatch() {
    let a = isolated_agent();
    let hello: HelloResult = a.ok(
        m::HELLO,
        serde_json::to_value(HelloParams {
            client: ClientKind::Desktop,
            version: "contract-test".into(),
        })
        .unwrap(),
    );
    assert_eq!(hello.protocol, PROTOCOL_VERSION);
    assert_eq!(hello.agent_version, env!("CARGO_PKG_VERSION"));

    // The transport sends null for omitted arguments; explicit {} must agree.
    for params in [Value::Null, json!({})] {
        let status: VaultStatus = a.ok(m::VAULT_STATUS, params.clone());
        assert!(status.locked && !status.initialized);
        assert_eq!(status.locks_in_secs, None);
        let vaults: Vec<KnownVault> = a.ok(m::VAULT_LIST, params.clone());
        assert!(vaults.is_empty());
        let sessions: Vec<BrowserSession> = a.ok(m::BROWSER_LIST, params.clone());
        assert!(sessions.is_empty());
        let settings: Settings = a.ok(m::SETTINGS_GET, params.clone());
        assert_eq!(settings.auto_lock_secs, 900);
        let locked: VaultStatus = a.ok(m::VAULT_LOCK, params);
        assert!(locked.locked && !locked.initialized);
    }

    // Inspect the real envelope: TestAgent::call deliberately normalizes an
    // absent result into null, which would hide a broken unit-result response.
    let response = a.agent.dispatch(
        &ClientCtx::desktop(),
        RpcRequest {
            id: 71,
            method: m::BROWSER_CLOSE.into(),
            params: serde_json::to_value(IdParams {
                id: "no-running-session".into(),
            })
            .unwrap(),
        },
    );
    assert_eq!(response.id, 71);
    assert!(response.error.is_none());
    assert_eq!(response.result, Some(Value::Null));
    serde_json::from_value::<()>(response.result.unwrap()).unwrap();
}

#[test]
fn unknown_and_malformed_requests_fail_without_echoing_payloads() {
    let a = isolated_agent();
    let unknown = "protocol.contract.unknown";
    assert!(!m::ALL.contains(&unknown));
    let response = a.agent.dispatch(
        &ClientCtx::desktop(),
        RpcRequest {
            id: 93,
            method: unknown.into(),
            params: json!({"passphrase": "payload-must-not-be-echoed"}),
        },
    );
    assert_eq!(response.id, 93);
    assert!(response.result.is_none());
    let error = response.error.expect("unknown method must fail");
    assert_eq!(error.code, ErrorCode::InvalidParams);
    assert!(
        error.message == format!("invalid parameters: unknown method `{unknown}`"),
        "unknown-method error must contain only the method name and static text"
    );

    let error = a
        .call(
            m::PROFILES_GET,
            json!({"id": ["payload-must-not-be-echoed"]}),
        )
        .expect_err("malformed known-method parameters must fail");
    assert_eq!(error.code, ErrorCode::InvalidParams);
    assert!(
        error.message == "invalid parameters: profiles.get: malformed parameters",
        "malformed-parameter error must contain only the method name and static text"
    );
}
