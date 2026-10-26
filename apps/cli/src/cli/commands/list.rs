use super::*;

pub(super) fn list(
    s: &mut Session,
    profile: Option<String>,
    entry_type: Option<String>,
    json: bool,
) -> CliResult {
    let pid = match &profile {
        Some(p) => Some(resolve_profile(s, p)?.id),
        None => None,
    };
    let et = entry_type.as_deref().map(parse_type).transpose()?;
    let list: Vec<EntrySummary> = s.entries_list(ListEntriesParams {
        profile_id: pid,
        query: None,
        entry_type: et,
    })?;
    print_entry_list(s, &list, json)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn propagates_missing_vault_without_losing_exit_policy() {
        let mut fixture = crate::cli::commands::test_support::EmptySession::new();
        let error = list(&mut fixture.session, None, None, true).unwrap_err();
        assert_eq!(error.code, 1);
        assert!(error.message.contains("latch init"), "{}", error.message);
    }
}
