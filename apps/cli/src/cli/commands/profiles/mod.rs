//! Profile management.
mod add;
mod list;
mod remove;
mod rename;
mod show;

use super::{CliError, CliResult, ProfileCmd, Session, confirm, print_json, resolve_profile};
use latch_rpc::{sdk::RpcSdk, types::*};

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
