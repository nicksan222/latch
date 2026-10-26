//! Clap command routing and session lifecycle. Action behavior lives in child modules.

mod actions;
mod agent;
mod browser;
mod completion;
mod copy;
mod doctor;
mod doctor_staged;
mod environment;
mod export;
mod get;
mod history;
mod hook;
mod import;
mod init;
mod list;
mod lock;
mod migrate;
mod mv;
mod profiles;
mod recipients;
mod remove;
mod resolve;
mod restore;
mod search;
mod set;
mod status;
mod support;
mod sync;
mod totp;
mod trash;
mod trust;
mod unlock;

#[cfg(test)]
mod test_support;

use super::entries::*;
use super::session::{Session, read_passphrase};
use super::*;
use latch::agent::client::AgentClient;
use latch::core::crypto;
use latch_rpc::{sdk::RpcSdk, types::*};
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::time::Duration;
use support::*;
use zeroize::Zeroizing;

pub fn run(cli: Cli) -> CliResult {
    match cli.cmd {
        Cmd::Agent(c) => return agent::agent_cmd(c),
        Cmd::Completion { shell } => return completion::completion(shell),
        Cmd::Migrate => return migrate::migrate(),
        Cmd::Hook { action, path } => {
            let mut session = Session::open_local();
            let result = hook::hook(&mut session, action, path);
            if let Some(agent) = session.local_agent() {
                agent.shutdown();
            }
            return result;
        }
        Cmd::Doctor {
            staged: true, json, ..
        } => {
            let mut session = Session::open_local();
            let result = doctor_staged::doctor_staged(&mut session, json);
            if let Some(agent) = session.local_agent() {
                agent.shutdown();
            }
            return result;
        }
        _ => {}
    }
    crypto::harden_process();
    let mut s = Session::open()?;
    let r = dispatch(&mut s, cli.cmd);
    if let Some(a) = s.local_agent() {
        a.shutdown();
    }
    r
}

fn dispatch(s: &mut Session, cmd: Cmd) -> CliResult {
    match cmd {
        Cmd::Init(a) => init::init(s, a),
        Cmd::Unlock {
            timeout,
            identity,
            passphrase_stdin,
        } => unlock::unlock(s, timeout, identity, passphrase_stdin),
        Cmd::Lock => lock::lock(s),
        Cmd::Status { json } => status::status(s, json),
        Cmd::Profile(c) => profiles::profile(s, c),
        Cmd::Ls {
            profile,
            entry_type,
            json,
        } => list::list(s, profile, entry_type, json),
        Cmd::Search { query, json } => search::search(s, query, json),
        Cmd::Get {
            target,
            field,
            json,
            reveal,
            yes,
        } => get::get(s, &target, field, json, reveal, yes),
        Cmd::Set(a) => set::set(s, a),
        Cmd::Rm { target } => remove::remove(s, target),
        Cmd::Mv { from, to } => mv::mv(s, &from, &to),
        Cmd::Env(a) => environment::env(s, a),
        Cmd::Action(c) => actions::action(s, c),
        Cmd::Exec {
            target,
            field,
            command,
        } => environment::exec_secret(s, &target, field, command),
        Cmd::Totp { target, copy } => totp::totp(s, target, copy),
        Cmd::Copy {
            target,
            field,
            clear,
        } => copy::copy(s, target, field, clear),
        Cmd::Browser(a) => browser::browser(s, a),
        Cmd::Sync {
            pull_only,
            push_only,
            set_remote,
            json,
        } => sync::sync(s, pull_only, push_only, set_remote, json),
        Cmd::Resolve { target, resolution } => resolve::resolve(s, &target, resolution),
        Cmd::History { target, json } => history::history(s, target, json),
        Cmd::Restore { target, commit } => restore::restore(s, target, commit),
        Cmd::Trash { profile, json } => trash::trash(s, profile, json),
        Cmd::Recipients(c) => recipients::recipients(s, c),
        Cmd::Import {
            format,
            file,
            profile,
            dry_run,
            skip_duplicates,
        } => import::import(s, &format, &file, profile, dry_run, skip_duplicates),
        Cmd::Export {
            format,
            out,
            profile,
            yes,
        } => export::export(s, &format, out, profile, yes),
        Cmd::Doctor {
            fix,
            rebuild_manifest,
            json,
            ..
        } => doctor::doctor(s, fix, rebuild_manifest, json),
        Cmd::Trust(c) => trust::trust_cmd(s, c),
        Cmd::Agent(_) | Cmd::Completion { .. } | Cmd::Migrate | Cmd::Hook { .. } => Ok(()),
    }
}
