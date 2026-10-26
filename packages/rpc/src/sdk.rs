//! Blocking adapters preserve the generated request and response types.
//! Transports own connection mechanics; no domain implementation is required.
pub trait RpcTransport {
    type Error: From<crate::errors::RpcError>;
    fn execute(&mut self, call: Call) -> Result<Reply, Self::Error>;
}
include!(concat!(env!("OUT_DIR"), "/latch_sdk.rs"));
