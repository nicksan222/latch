//! Isolated browser launch and session management.
mod close;
mod list;
mod open;
mod reset;

use super::{BrowserArgs, BrowserCmd, CliError, CliResult, Session, print_json, resolve_profile};
use latch_rpc::{sdk::RpcSdk, types::*};

pub(super) fn browser(s: &mut Session, a: BrowserArgs) -> CliResult {
    match a.cmd {
        Some(BrowserCmd::Ls { json }) => list::list(s, json),
        Some(BrowserCmd::Close { profile }) => close::close(s, profile),
        Some(BrowserCmd::Reset { profile }) => reset::reset(s, profile),
        None => open::open(s, a),
    }
}
