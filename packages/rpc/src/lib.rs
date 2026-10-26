//! Standalone, typed Latch gRPC interface. The `.proto` file is the wire authority.
//! `types` retains transitional serde DTOs for application-facing adapters.

//! A consumer imports only this interface crate and receives an authenticated
//! channel from its local connector (never a backend implementation object).
//!
//! ```no_run
//! use latch_rpc::{client, codec, types::{ClientKind, HelloParams}, v1::{Empty, VaultStatus, latch_agent_client::LatchAgentClient}};
//!
//! async fn status(client: &mut LatchAgentClient<tonic::transport::Channel>) -> Result<VaultStatus, tonic::Status> {
//!     let identity = HelloParams { client: ClientKind::Cli, version: "example/1".into() };
//!     let request = client::request(Empty {}, &identity).map_err(|error| codec::status(&error))?;
//!     Ok(client.vault_status(request).await?.into_inner())
//! }
//! ```
//!

pub mod client;
pub mod codec;
pub mod sdk;
pub mod types;

pub mod v1 {
    tonic::include_proto!("latch.v1");
    include!(concat!(env!("OUT_DIR"), "/latch_debug.rs"));
}

/// Compiled protobuf descriptor set for schema-driven clients and codecs.
pub const DESCRIPTOR_SET: &[u8] = tonic::include_file_descriptor_set!("latch_descriptor");

include!(concat!(env!("OUT_DIR"), "/latch_methods.rs"));
