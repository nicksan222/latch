#[path = "../../../tests/agent_support/mod.rs"]
mod agent_support;

use latch::rpc::{ClientCtx, ErrorCode, LocalClient, RpcSdk, RpcTransport, methods};
use serde::Serialize;
use serde_json::{Value, json};

#[test]
fn public_client_dispatches_typed_and_dynamic_calls() {
    let fixture = agent_support::TestAgent::new();
    let mut client = LocalClient::new(&fixture.agent, ClientCtx::desktop());
    assert!(!client.vault_status().unwrap().initialized);
    let closed = client.browser_close(latch::rpc::IdParams { id: "absent".into() });
    assert!(closed.is_ok(), "unit-returning procedures must decode null");
    let error = client.call(methods::PROFILES_LIST, Value::Null).unwrap_err();
    assert_eq!(error.code, ErrorCode::NoVault);
    let error = client.call(methods::PROFILES_GET, json!({"id": ["rpc-test-private-marker"]})).unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidParams);
    assert!(!error.message.contains("rpc-test-private-marker"));
}

#[test]
fn serialization_and_response_shape_failures_never_echo_values() {
    struct BrokenParams;
    impl Serialize for BrokenParams {
        fn serialize<S: serde::Serializer>(&self, _serializer: S) -> Result<S::Ok, S::Error> {
            Err(serde::ser::Error::custom("rpc-test-private-marker"))
        }
    }
    let fixture = agent_support::TestAgent::new();
    let mut client = LocalClient::new(&fixture.agent, ClientCtx::cli());
    let result: Result<Value, _> = client.request(methods::VAULT_STATUS, BrokenParams);
    let error = result.unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidParams);
    assert!(!error.message.contains("rpc-test-private-marker"));
    // A consumer selecting the wrong result type receives a sanitized local error.
    let result: Result<Vec<String>, _> = client.request(methods::VAULT_STATUS, ());
    assert_eq!(result.unwrap_err().code, ErrorCode::Internal);
}
