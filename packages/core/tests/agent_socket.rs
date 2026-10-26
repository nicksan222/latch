// Cargo discovery only; acceptance tests live with their behavior owner.
#![cfg(unix)]

include!("../src/agent/server/tests/agent_socket.rs");
