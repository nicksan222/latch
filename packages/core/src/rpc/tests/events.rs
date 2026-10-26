// Generated gRPC WatchEvents streams stay active alongside unary requests.
#[path = "../../../tests/agent_support/mod.rs"]
mod agent_support;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use agent_support::TestAgent;
use latch::rpc::{AgentClient, AgentEventKind, ClientKind, HelloParams, VaultStatus, methods};

static SOCKET_SERIAL: Mutex<()> = Mutex::new(());

fn hello() -> HelloParams {
    HelloParams {
        client: ClientKind::Cli,
        version: "grpc-events-test".into(),
    }
}

fn expect_kind(subscription: &latch::agent::client::EventSubscription, kind: AgentEventKind) {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        assert!(!remaining.is_zero(), "event {kind:?} was not received");
        let event = subscription.recv_timeout(remaining).unwrap().unwrap();
        if event.kind == kind {
            return;
        }
    }
}

fn scenario(mut client: AgentClient, agent: &TestAgent) {
    let subscription = client.watch_events().unwrap();
    let before: VaultStatus = serde_json::from_value(
        client
            .call(methods::VAULT_STATUS, serde_json::Value::Null)
            .unwrap(),
    )
    .unwrap();
    assert!(!before.initialized);
    agent.init();
    expect_kind(&subscription, AgentEventKind::Unlocked);
    agent.agent.lock();
    expect_kind(&subscription, AgentEventKind::Locked);
    drop(subscription);
    let after: VaultStatus = serde_json::from_value(
        client
            .call(methods::VAULT_STATUS, serde_json::Value::Null)
            .unwrap(),
    )
    .unwrap();
    assert!(after.locked);
}

#[test]
fn embedded_generated_event_stream_and_unary_calls() {
    let agent = TestAgent::new();
    let (client, _) = AgentClient::connect_embedded(Arc::downgrade(&agent.agent), hello()).unwrap();
    scenario(client, &agent);
}

#[cfg(unix)]
#[test]
fn socket_generated_event_stream_and_unary_calls() {
    let _guard = SOCKET_SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let agent = TestAgent::new();
    agent.agent.serve_socket().unwrap();
    let (client, _) = AgentClient::connect(hello()).unwrap();
    scenario(client, &agent);
    agent.agent.shutdown();
}
