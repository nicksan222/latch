//! Latch: local, git-backed secret manager.
//!
//! - `core`     vault format, crypto, git, entries, import/export, browser
//! - `agent`    unlocked-key holder, RPC dispatcher, socket server, policy
//! - `rpc`      backend host adapters for the standalone `latch-rpc` interface
//! - `protocol` internal dispatcher envelopes and interface-model reexports

pub mod agent;
pub mod core;
pub mod protocol;

pub mod rpc;
