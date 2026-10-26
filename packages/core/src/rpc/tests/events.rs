// Generated gRPC WatchEvents streams stay active alongside unary requests.
#[path = "../../../tests/agent_support/mod.rs"]
mod agent_support;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use agent_support::TestAgent;
use latch::protocol::{AgentEventKind, VaultStatus};
use latch::rpc::{AgentClient, Call, Reply, v1};

static SOCKET_SERIAL: Mutex<()> = Mutex::new(());

fn hello() -> v1::HelloParams {
    v1::HelloParams {
        client: v1::ClientKind::Cli as i32,
        version: "grpc-events-test".into(),
    }
}

fn status(client: &mut AgentClient) -> VaultStatus {
    let Reply::VaultStatus(status) = client.execute(Call::VaultStatus(v1::Empty {})).unwrap()
    else {
        panic!("vault.status returned a different typed reply")
    };
    VaultStatus::try_from(status).unwrap()
}

fn expect_kind(subscription: &latch::agent::client::EventSubscription, kind: AgentEventKind) {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        assert!(!remaining.is_zero(), "event {kind:?} was not received");
        let event = subscription.recv_timeout(remaining).unwrap().unwrap();
        if AgentEventKind::try_from(event.kind).unwrap() == kind {
            return;
        }
    }
}

fn scenario(mut client: AgentClient, agent: &TestAgent) {
    let subscription = client.watch_events().unwrap();
    let before = status(&mut client);
    assert!(!before.initialized);
    agent.init();
    expect_kind(&subscription, AgentEventKind::Unlocked);
    agent.agent.lock();
    expect_kind(&subscription, AgentEventKind::Locked);
    drop(subscription);
    let after = status(&mut client);
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
