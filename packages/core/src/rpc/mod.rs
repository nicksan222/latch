//! Core host adapters for the standalone Protobuf/gRPC interface.
//!
//! Import [`RpcSdk`] for typed procedures and choose a transport:
//! [`LocalClient`] for an embedded [`Agent`], [`AgentClient`] for its authenticated
//! Unix socket, or implement [`RpcTransport`] for an application adapter. All
//! request/result DTOs, [`methods`], errors and events are exported here from the
//! authored `packages/rpc/proto/latch/v1/agent.proto` contract. No vault behavior belongs in an
//! adapter. The generated methods are identical for every Rust consumer.
//!
//! ```no_run
//! use latch::rpc::{Agent, AgentOptions, ClientCtx, LocalClient, RpcSdk};
//!
//! let agent = Agent::new(AgentOptions::default());
//! let mut rpc = LocalClient::new(&agent, ClientCtx::desktop());
//! let status = rpc.vault_status()?;
//! assert!(status.locked);
//! agent.shutdown();
//! # Ok::<(), latch::rpc::RpcError>(())
//! ```
//!
//! Embedded contexts are trusted application identities, not authentication for
//! remote peers. Socket connections authenticate through [`AgentClient::connect`]
//! and `hello`; never accept a caller-supplied embedded context across a network.
//! Calls block on KDF/Git/filesystem work; async shells must use a blocking pool.
//! Clients subscribe through [`LocalClient::watch_events`] and the generated
//! `WatchEvents` stream; adapters forward them to their UI.
//! The socket client currently supports Unix only; Windows named pipes are future
//! work. [`AgentClient::connect_or_spawn`] explicitly opts into starting the CLI
//! agent, while [`AgentClient::connect`] never starts another process.

pub use crate::agent::client::{AgentClient, EventSubscription};
pub use crate::agent::{Agent, AgentOptions, ClientCtx};
pub use latch_rpc::sdk::{RpcSdk, RpcTransport};
mod service;
pub use crate::protocol::*;
pub(crate) use service::GrpcService;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

/// Synchronous compatibility adapter to the generated gRPC client of an embedded agent.
/// It owns a private authenticated in-memory Unix connection, never a direct dispatcher.
pub struct LocalClient {
    client: Result<AgentClient, RpcError>,
}

impl LocalClient {
    pub fn new(agent: &std::sync::Arc<Agent>, context: ClientCtx) -> Self {
        let hello = HelloParams {
            client: context.kind,
            version: env!("CARGO_PKG_VERSION").into(),
        };
        let client = AgentClient::connect_embedded(std::sync::Arc::downgrade(agent), hello)
            .map(|(client, _)| client);
        Self { client }
    }

    pub fn watch_events(&mut self) -> Result<EventSubscription, RpcError> {
        self.client
            .as_mut()
            .map_err(|error| error.clone())?
            .watch_events()
    }

    pub fn call(&mut self, method: &str, params: Value) -> Result<Value, RpcError> {
        self.client
            .as_mut()
            .map_err(|error| error.clone())?
            .call(method, params)
    }
}

fn encode<P: Serialize>(params: P) -> Result<Value, RpcError> {
    serde_json::to_value(params).map_err(|_| RpcError {
        code: ErrorCode::InvalidParams,
        message: "cannot encode RPC parameters".into(),
    })
}

fn decode<R: DeserializeOwned>(result: Value) -> Result<R, RpcError> {
    serde_json::from_value(result).map_err(|_| RpcError {
        code: ErrorCode::Internal,
        message: "unexpected RPC response shape".into(),
    })
}

impl RpcTransport for LocalClient {
    type Error = RpcError;

    fn request<P: Serialize, R: DeserializeOwned>(
        &mut self,
        method: &str,
        params: P,
    ) -> Result<R, Self::Error> {
        decode(self.call(method, encode(params)?)?)
    }
}

impl RpcTransport for AgentClient {
    type Error = RpcError;

    fn request<P: Serialize, R: DeserializeOwned>(
        &mut self,
        method: &str,
        params: P,
    ) -> Result<R, Self::Error> {
        decode(self.call(method, encode(params)?)?)
    }
}
