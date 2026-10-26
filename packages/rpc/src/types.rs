//! Transitional application DTOs generated from the protobuf descriptor.
//! Protobuf is the sole wire schema; these serde models preserve existing
//! presentation and command code while consumers move to generated clients.

include!(concat!(env!("OUT_DIR"), "/latch_compat_types.rs"));

pub const PROTOCOL_VERSION: u32 = 1;

impl std::fmt::Display for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RPC error ({:?})", self.code)
    }
}

impl std::error::Error for RpcError {}
