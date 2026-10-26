// Cargo discovery only; external gRPC acceptance lives with the RPC owner.
#![cfg(unix)]
include!("../src/rpc/tests/external/mod.rs");
