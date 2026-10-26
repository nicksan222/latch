//! Core hosting and connection adapters for the standalone Protobuf interface.
//! Applications import messages and clients from `latch_rpc`.
pub use crate::agent::client::{AgentClient, EventSubscription};
pub use crate::agent::{Agent, AgentOptions, ClientCtx};
pub use latch_rpc::{
    errors::RpcError,
    sdk::{Call, Reply, RpcSdk, RpcTransport},
    v1,
};
mod service;
pub(crate) use service::GrpcService;

/// A blocking generated client on a private authenticated embedded gRPC channel.
pub struct LocalClient {
    client: Result<AgentClient, RpcError>,
}
impl LocalClient {
    pub fn new(agent: &std::sync::Arc<Agent>, context: ClientCtx) -> Self {
        let identity = v1::HelloParams {
            client: if matches!(context.kind, crate::protocol::ClientKind::Cli) {
                v1::ClientKind::Cli as i32
            } else {
                v1::ClientKind::Desktop as i32
            },
            version: env!("CARGO_PKG_VERSION").into(),
        };
        let client = AgentClient::connect_embedded(std::sync::Arc::downgrade(agent), identity)
            .map(|(client, _)| client);
        Self { client }
    }
    pub fn watch_events(&mut self) -> Result<EventSubscription, RpcError> {
        self.client
            .as_mut()
            .map_err(|error| error.clone())?
            .watch_events()
    }
    pub fn execute(&mut self, call: Call) -> Result<Reply, RpcError> {
        self.client
            .as_mut()
            .map_err(|error| error.clone())?
            .execute(call)
    }
}
impl RpcTransport for LocalClient {
    type Error = RpcError;
    fn execute(&mut self, call: Call) -> Result<Reply, Self::Error> {
        LocalClient::execute(self, call)
    }
}
impl RpcTransport for AgentClient {
    type Error = RpcError;
    fn execute(&mut self, call: Call) -> Result<Reply, Self::Error> {
        AgentClient::execute(self, call)
    }
}
