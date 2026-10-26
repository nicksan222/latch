//! Environment formatting and child execution with resolved secrets.
mod env;
mod exec;

pub(super) fn env(s: &mut Session, a: EnvArgs) -> CliResult {
    env::env(s, a)
}

pub(super) fn exec_secret(
    s: &mut Session,
    target: &str,
    field: Option<String>,
    command: Vec<String>,
) -> CliResult {
    exec::exec_secret(s, target, field, command)
}

use super::{
    CliError, CliResult, EnvArgs, EnvFormatArg, Session, default_field, find_in, list_entries,
    resolve_entry, resolve_profile,
};
use latch_rpc::{sdk::RpcSdk, types::*};
use zeroize::Zeroizing;

/// Replace the process with `cmd` (Unix) or run it and propagate its exit
/// code. Inherited stdio.
fn run_child(s: &mut Session, mut cmd: std::process::Command) -> CliResult {
    if let Some(a) = s.local_agent() {
        a.shutdown();
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = cmd.exec();
        Err(CliError::new(127, format!("cannot run command: {err}")))
    }
    #[cfg(not(unix))]
    {
        let st = cmd
            .status()
            .map_err(|e| CliError::new(127, format!("cannot run command: {e}")))?;
        std::process::exit(st.code().unwrap_or(1));
    }
}
