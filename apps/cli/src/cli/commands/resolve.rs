use super::*;

pub(super) fn resolve(s: &mut Session, target: &str, res: ResolutionArg) -> CliResult {
    let (pname, ename) = split_target(target)?;
    let st: SyncStatus = s.sync_status()?;
    let profiles: Vec<ProfileSummary> = s.profiles_list().unwrap_or_default();
    let pid = profiles
        .iter()
        .find(|p| p.id == pname || p.name.eq_ignore_ascii_case(pname))
        .map(|p| p.id.clone())
        .unwrap_or_else(|| pname.to_owned());
    let c = st
        .conflicts
        .iter()
        .find(|c| {
            c.profile_id == pid && (c.entry_id == ename || c.name.eq_ignore_ascii_case(ename))
        })
        .ok_or_else(|| CliError::user(format!("no conflict for `{target}`")))?;
    let st: SyncStatus = s.sync_resolve(ResolveParams {
        entry_id: c.entry_id.clone(),
        resolution: match res {
            ResolutionArg::Mine => Resolution::Mine,
            ResolutionArg::Theirs => Resolution::Theirs,
        },
        merged: None,
    })?;
    if st.conflicts.is_empty() {
        eprintln!("resolved; sync complete (run `latch sync` to push)");
    } else {
        eprintln!("resolved; {} conflict(s) remaining", st.conflicts.len());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_target_is_rejected_before_reading_conflicts() {
        let mut fixture = crate::cli::commands::test_support::EmptySession::new();
        let error = resolve(&mut fixture.session, "malformed", ResolutionArg::Mine).unwrap_err();
        assert_eq!(error.code, 1);
        assert!(error.message.contains("<profile>/<entry>"));
    }
}
