use super::*;

pub(super) fn restore(s: &mut Session, target: String, commit: String) -> CliResult {
    let (pname, ename) = split_target(&target)?;
    let prof = resolve_profile(s, pname)?;
    let list = list_entries(s, &prof.id)?;
    let id = match find_in(&list, ename) {
        Some(e) => e.id,
        None => {
            let trash = s
                .entries_trash(TrashParams {
                    profile_id: Some(prof.id.clone()),
                })?
                .items;
            trash
                .iter()
                .find(|t| t.id == ename || t.name.eq_ignore_ascii_case(ename))
                .map(|t| t.id.clone())
                .ok_or_else(|| CliError::user(format!("no entry `{ename}`")))?
        }
    };
    let e: Entry = s.entries_restore(RestoreParams { id, commit })?;
    eprintln!("restored {}/{}", prof.name, e.name);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn propagates_missing_vault_without_losing_exit_policy() {
        let mut fixture = crate::cli::commands::test_support::EmptySession::new();
        let error = restore(&mut fixture.session, "work/key".into(), "HEAD".into()).unwrap_err();
        assert_eq!(error.code, 1);
        assert!(error.message.contains("latch init"), "{}", error.message);
    }
}
