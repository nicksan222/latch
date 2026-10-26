//! Binding the agent socket from the desktop app (SPEC §4.2: the desktop
//! app, when running, *is* the agent).
//!
//! If another agent already holds `agent.lock`:
//! - a headless `latch agent run` (spawned on demand by the CLI / native
//!   host; Linux: identified by `/proc/<pid>/comm == "latch"`) is asked to
//!   stop with SIGTERM — the same thing `latch agent stop` does: it locks,
//!   closes its browsers and unbinds — and the desktop app takes over the
//!   socket. Anything unlocked there must be unlocked again in the app.
//! - anything else (unknown owner) is left alone; the app runs without the
//!   socket (the CLI keeps talking to the other agent) and tells the
//!   user.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use latch::core::Error;
use latch::rpc::Agent;
use latch_rpc::v1::ErrorCode;

#[derive(Debug)]
pub enum SocketOutcome {
    Bound(PathBuf),
    /// Took over from a headless agent (its pid).
    TookOver(PathBuf, u32),
    /// Running without a socket; the message is user-facing.
    Unavailable(String),
}

pub fn bind_or_take_over(agent: &Arc<Agent>) -> SocketOutcome {
    match agent.serve_socket() {
        Ok(p) => SocketOutcome::Bound(p),
        Err(e) if e.code() == ErrorCode::AlreadyExists => take_over(agent),
        Err(e) => SocketOutcome::Unavailable(unavailable_msg(&e)),
    }
}

fn unavailable_msg(e: &Error) -> String {
    format!("The CLI cannot reach this app: {e}")
}

/// Pid of the running agent, if the pid file names a live headless CLI agent.
fn headless_agent_pid() -> Option<u32> {
    let pid: u32 = std::fs::read_to_string(latch::agent::pid_path())
        .ok()?
        .trim()
        .parse()
        .ok()?;
    if pid == std::process::id() {
        return None;
    }
    is_headless_cli(pid).then_some(pid)
}

#[cfg(target_os = "linux")]
fn is_headless_cli(pid: u32) -> bool {
    std::fs::read_to_string(format!("/proc/{pid}/comm"))
        .map(|c| c.trim() == "latch")
        .unwrap_or(false)
}

#[cfg(all(unix, not(target_os = "linux")))]
fn is_headless_cli(pid: u32) -> bool {
    // No cheap process-name lookup without extra deps: trust the pid file
    // only when the process exists.
    // SAFETY: kill(pid, 0) only checks existence/permissions.
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

#[cfg(not(unix))]
fn is_headless_cli(_pid: u32) -> bool {
    false
}

fn take_over(agent: &Arc<Agent>) -> SocketOutcome {
    let Some(pid) = headless_agent_pid() else {
        return SocketOutcome::Unavailable(
            "Another Latch agent is already running; the CLI talks to it, \
             not to this window. Quit it (`latch agent stop`) and restart Latch."
                .into(),
        );
    };
    #[cfg(unix)]
    // SAFETY: plain kill(2) on the pid of our own headless agent, read from
    // the private (0700) runtime dir and checked above.
    unsafe {
        libc::kill(pid as i32, libc::SIGTERM);
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match agent.serve_socket() {
            Ok(p) => return SocketOutcome::TookOver(p, pid),
            Err(e) if e.code() == ErrorCode::AlreadyExists && Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => return SocketOutcome::Unavailable(unavailable_msg(&e)),
        }
    }
}
