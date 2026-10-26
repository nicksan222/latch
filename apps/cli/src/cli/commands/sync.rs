use super::*;

pub(super) fn sync(
    s: &mut Session,
    pull_only: bool,
    push_only: bool,
    set_remote: Option<String>,
    json: bool,
) -> CliResult {
    if let Some(url) = set_remote {
        let st: SyncStatus = s.sync_set_remote(SetRemoteParams { url })?;
        eprintln!(
            "remote: {}",
            st.remote_url.unwrap_or_else(|| "(none)".into())
        );
        return Ok(());
    }
    let mode = if pull_only {
        SyncMode::PullOnly
    } else if push_only {
        SyncMode::PushOnly
    } else {
        SyncMode::Both
    };
    let r: SyncResult = s.sync_run(SyncParams { mode: mode as i32 })?;
    if json {
        print_proto_json(&r, "SyncResult")?;
    } else {
        eprintln!(
            "pulled {} commit(s), pushed {} commit(s)",
            r.pulled, r.pushed
        );
    }
    if !r.conflicts.is_empty() {
        let lines = conflict_lines(s, &r.conflicts);
        if !json {
            outln!("conflicts:");
            for l in &lines {
                outln!("  {l}");
            }
        }
        return Err(CliError::new(
            4,
            "resolve each conflict with `latch resolve <profile>/<entry> mine|theirs`",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn propagates_missing_vault_without_losing_exit_policy() {
        let mut fixture = crate::cli::commands::test_support::EmptySession::new();
        let error = sync(&mut fixture.session, false, false, None, true).unwrap_err();
        assert_eq!(error.code, 1);
        assert!(error.message.contains("latch init"), "{}", error.message);
    }
}
