//! Profile management.
mod add;
mod list;
mod remove;
mod rename;
mod show;

use super::{CliError, CliResult, ProfileCmd, Session, confirm, resolve_profile};
use crate::cli::entries::{print_proto_json, print_proto_json_list};
use latch_rpc::sdk::RpcSdk;
use latch_rpc::v1::*;

pub(super) fn profile(s: &mut Session, c: ProfileCmd) -> CliResult {
    match c {
        ProfileCmd::Ls { json } => list::list(s, json),
        ProfileCmd::Add {
            name,
            color,
            icon,
            description,
        } => add::add(s, name, color, icon, description),
        ProfileCmd::Rm { name, yes } => remove::remove(s, name, yes),
        ProfileCmd::Rename { old, new } => rename::rename(s, old, new),
        ProfileCmd::Show { name, json } => show::show(s, name, json),
    }
}
