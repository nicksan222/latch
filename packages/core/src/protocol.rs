//! Core domain models and their protobuf boundary conversions.
mod conversions;
mod models;
pub use latch_rpc::PROTOCOL_VERSION;
pub use latch_rpc::errors::RpcError;
pub use latch_rpc::v1::ErrorCode;
pub use models::*;
