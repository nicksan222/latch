#[path = "../../../tests/agent_support/mod.rs"]
mod agent_support;

use latch::protocol::ErrorCode;
use latch::rpc::{Call, ClientCtx, LocalClient, RpcSdk, v1};

#[test]
fn public_client_executes_typed_calls_and_preserves_unit_replies() {
    let fixture = agent_support::TestAgent::new();
    let mut client = LocalClient::new(&fixture.agent, ClientCtx::desktop());
    assert!(!client.vault_status().unwrap().initialized);
    let closed = client
        .browser_close(v1::IdParams {
            id: "absent".into(),
        })
        .expect("unit-returning procedures return an Empty message");
    assert_eq!(closed, v1::Empty {});
    let error = client.profiles_list().unwrap_err();
    assert_eq!(error.code, ErrorCode::NoVault);
}

#[test]
fn invalid_typed_enum_values_fail_without_echoing_values() {
    let fixture = agent_support::TestAgent::new();
    let mut client = LocalClient::new(&fixture.agent, ClientCtx::cli());
    let error = client
        .execute(Call::Hello(v1::HelloParams {
            client: 99,
            version: "rpc-test-private-marker".into(),
        }))
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidParams);
    assert!(!error.message.contains("rpc-test-private-marker"));
}
