// Black-box acceptance: this process uses only the generated interface client.
// The sole backend import is in host.rs, executed in a separate child process.
mod host;
mod support;
use support::*;
mod domain;
mod integrations;
mod kinds;
mod lifecycle;
mod maintenance;
mod sync;
mod transport;

mod native_browser;
