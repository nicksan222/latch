//! Isolated browser launch and session management.
mod close;
mod list;
mod open;
mod reset;

use super::{BrowserArgs, BrowserCmd, CliError, CliResult, Session, resolve_profile};
use crate::cli::entries::print_proto_json_list;
use latch_rpc::sdk::RpcSdk;
use latch_rpc::v1::*;

pub(super) fn browser(s: &mut Session, a: BrowserArgs) -> CliResult {
    match a.cmd {
        Some(BrowserCmd::Ls { json }) => list::list(s, json),
        Some(BrowserCmd::Close { profile }) => close::close(s, profile),
        Some(BrowserCmd::Reset { profile }) => reset::reset(s, profile),
        None => open::open(s, a),
    }
}
