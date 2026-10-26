// Stateless domain RPCs are usable before a vault exists and while locked.
#[path = "../../../../tests/agent_support/mod.rs"]
mod agent_support;
use agent_support::TestAgent;
use latch::agent::ClientCtx;
use latch::protocol::{DomainRequest, DomainResponse, ErrorCode, methods as m};
use serde_json::json;

#[test]
fn typed_domain_rpc_is_available_without_a_vault() {
    let agent = TestAgent::new();
    for ctx in [ClientCtx::desktop(), ClientCtx::cli()] {
        let value = agent
            .call_as(
                &ctx,
                m::DOMAIN_CALL,
                serde_json::to_value(DomainRequest::GeneratePassword(24)).unwrap(),
            )
            .unwrap();
        let DomainResponse::GeneratePassword(password) = serde_json::from_value(value).unwrap()
        else {
            panic!("response kind")
        };
        assert_eq!(password.len(), 24);
    }
    let error = agent
        .call(
            m::DOMAIN_CALL,
            json!({"operation":"unknown","input":"private-value"}),
        )
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidParams);
    assert!(!error.message.contains("private-value"));
}
