//! Latch's standalone Protobuf contract, generated clients, and transport helpers.
//! Consumers depend on this crate, never on the core implementation.

pub mod client;
pub mod errors;
pub mod sdk;
pub use errors::RpcError;
pub const PROTOCOL_VERSION: u32 = 1;
pub mod v1 {
    tonic::include_proto!("latch.v1");
    include!(concat!(env!("OUT_DIR"), "/latch_debug.rs"));
}
/// Compiled service descriptor for tooling and contract checks.
pub const DESCRIPTOR_SET: &[u8] = tonic::include_file_descriptor_set!("latch_descriptor");
include!(concat!(env!("OUT_DIR"), "/latch_methods.rs"));
