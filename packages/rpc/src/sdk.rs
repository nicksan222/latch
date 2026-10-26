//! Application-facing synchronous SDK generated from the protobuf service.
//! Transports implement this trait to route typed procedures over their own
//! connection lifecycle; the method registry lives only in the descriptor.

use serde::Serialize;
use serde::de::DeserializeOwned;

pub trait RpcTransport {
    type Error;
    fn request<P: Serialize, R: DeserializeOwned>(
        &mut self,
        method: &str,
        params: P,
    ) -> Result<R, Self::Error>;
}

include!(concat!(env!("OUT_DIR"), "/latch_sdk.rs"));
