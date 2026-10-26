use super::super::entries::*;
use super::super::session::Session;
use super::super::{CliError, CliResult};
use latch_rpc::sdk::methods as m;
use latch_rpc::{sdk::RpcSdk, types::*};
use serde_json::Value;
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub(super) fn null() -> Value {
    Value::Null
}

pub(super) fn confirm(question: &str) -> bool {
    if !std::io::stdin().is_terminal() {
        return false;
    }
    eprint!("{question} [y/N] ");
    let _ = std::io::stderr().flush();
    let mut s = String::new();
    std::io::stdin().read_line(&mut s).is_ok() && matches!(s.trim(), "y" | "Y" | "yes")
}

pub(super) fn trust_state_name(t: &TrustStatus) -> &'static str {
    match t.state {
        VaultTrustState::FirstUse => "first use (not yet verified)",
        VaultTrustState::Pinned => "pinned",
        VaultTrustState::Stale => "stale (sync to get the latest key rotation)",
    }
}

pub(super) fn trust_notice(st: &VaultStatus) {
    if let Some(t) = &st.trust
        && t.state == VaultTrustState::FirstUse
    {
        eprintln!("first unlock of this vault on this machine; its keys are now trusted.");
        eprintln!("vault fingerprint: {}", t.fingerprint);
        eprintln!(
            "compare it on another device (`latch status`) or with your recovery kit; if it differs, run `latch trust reset` and do not use this copy"
        );
    }
}

pub(super) fn locks_in(st: &VaultStatus) -> String {
    match st.locks_in_secs {
        Some(s) => format!(" (auto-lock in {} min)", s.div_ceil(60)),
        None => String::new(),
    }
}

pub(super) fn integrity_probe(s: &mut Session) -> CliResult {
    // Inspect the original RPC error without prompting or retrying an unlock.
    match s.raw(m::PROFILES_LIST, null()) {
        Err(e) if e.code == ErrorCode::Integrity => Err(CliError::new(
            3,
            format!(
                "integrity problem: {} — data access is blocked; run `latch doctor`",
                e.message
            ),
        )),
        _ => Ok(()),
    }
}

pub(super) fn absolute(p: &Path) -> CliResult<PathBuf> {
    std::path::absolute(p).map_err(|_| CliError::user("cannot resolve absolute path"))
}

pub(super) fn print_entry_list(s: &mut Session, list: &[EntrySummary], json: bool) -> CliResult {
    if json {
        return print_json(&list);
    }
    let profiles: Vec<ProfileSummary> = s.profiles_list()?;
    for e in list {
        let pname = profiles
            .iter()
            .find(|p| p.id == e.profile_id)
            .map(|p| p.name.as_str())
            .unwrap_or("?");
        outln!(
            "{}/{}  [{}]{}",
            pname,
            e.name,
            type_name(e.entry_type),
            e.subtitle
                .as_deref()
                .map(|x| format!("  {x}"))
                .unwrap_or_default()
        );
    }
    Ok(())
}

pub(super) fn copied(s: &mut Session, secs: u32) -> CliResult {
    if secs == 0 {
        eprintln!("copied to clipboard");
        return Ok(());
    }
    eprintln!("copied to clipboard; clears in {secs} s");
    if s.is_standalone() {
        // No agent will outlive us: keep serving the clipboard, then clear.
        eprintln!("(standalone mode: waiting to clear; Ctrl-C to exit)");
        std::thread::sleep(Duration::from_secs(secs as u64));
    }
    Ok(())
}

pub(super) fn conflict_lines(s: &mut Session, conflicts: &[ConflictSummary]) -> Vec<String> {
    let profiles: Vec<ProfileSummary> = s.profiles_list().unwrap_or_default();
    conflicts
        .iter()
        .map(|c| {
            let p = profiles
                .iter()
                .find(|p| p.id == c.profile_id)
                .map(|p| p.name.clone())
                .unwrap_or(c.profile_id.clone());
            format!("{p}/{}", c.name)
        })
        .collect()
}

pub(super) fn print_doctor(r: &DoctorReport, json: bool) -> CliResult {
    if json {
        return print_json(r);
    }
    for i in &r.issues {
        eprintln!(
            "{:?}: {}: {}{}",
            i.severity,
            i.code,
            i.message,
            i.path
                .as_deref()
                .map(|p| format!(" ({p})"))
                .unwrap_or_default()
        );
    }
    for f in &r.fixed {
        eprintln!("fixed: {f}");
    }
    if r.issues.is_empty() {
        eprintln!("ok: no issues");
    }
    Ok(())
}

pub(super) fn local_vault_root(s: &mut Session) -> CliResult<PathBuf> {
    let cwd = std::env::current_dir()
        .map(|path| path.display().to_string())
        .unwrap_or_default();
    let result = s.vault_locate(OpenParams { path: cwd })?;
    Ok(PathBuf::from(result.text))
}

pub(super) fn print_recipients(recipients: &[RecipientInfo]) {
    for recipient in recipients {
        outln!(
            "{}  {:?}  {}",
            recipient.label,
            recipient.kind,
            recipient.public_key
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absolute_paths_allow_nonexistent_destinations() {
        let relative = Path::new("nonexistent-latch-parent/nonexistent-vault");
        assert_eq!(
            absolute(relative).unwrap(),
            std::env::current_dir().unwrap().join(relative)
        );
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("missing/vault");
        assert_eq!(absolute(&destination).unwrap(), destination);
        assert!(!destination.exists());
    }

    #[test]
    fn absolute_path_errors_are_safe() {
        let error = absolute(Path::new("")).unwrap_err();
        assert_eq!(error.code, 1);
        assert_eq!(error.message, "cannot resolve absolute path");
    }

    #[cfg(unix)]
    #[test]
    fn absolute_paths_preserve_symlinks_and_parent_components() {
        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("nested/target");
        std::fs::create_dir_all(&target).unwrap();
        let link = directory.path().join("link");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let path = link.join("..");
        let resolved = absolute(&path).unwrap();
        assert_eq!(resolved, path);
        assert_eq!(resolved.canonicalize().unwrap(), target.parent().unwrap());
        assert_ne!(resolved.canonicalize().unwrap(), directory.path());
    }
}
