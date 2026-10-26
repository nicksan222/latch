use super::*;

pub(super) fn mv(s: &mut Session, from: &str, to: &str) -> CliResult {
    let (_, e) = resolve_entry(s, from)?;
    let (tp, tname) = match to.split_once('/') {
        Some((p, n)) => (p, n),
        None => (to, ""),
    };
    let target = resolve_profile(s, tp)?;
    let entry = get_entry(s, &e.id, false)?;
    let mut input = input_from_entry(&entry);
    input.profile_id = target.id.clone();
    if !tname.trim().is_empty() {
        input.name = tname.trim().to_owned();
    }
    let r: Entry = s.entries_update(EntryUpdateParams {
        id: e.id,
        input: Some(input),
    })?;
    eprintln!("moved to {}/{}", target.name, r.name);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn propagates_missing_vault_without_losing_exit_policy() {
        let mut fixture = crate::cli::commands::test_support::EmptySession::new();
        let error = mv(&mut fixture.session, "work/key", "other/new").unwrap_err();
        assert_eq!(error.code, 1);
        assert!(error.message.contains("latch init"), "{}", error.message);
    }
}
