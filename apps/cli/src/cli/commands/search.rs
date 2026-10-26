use super::*;

pub(super) fn search(s: &mut Session, query: String, json: bool) -> CliResult {
    let list = s
        .entries_list(ListEntriesParams {
            profile_id: None,
            query: Some(query),
            r#type: None,
        })?
        .items;
    print_entry_list(s, &list, json)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn propagates_missing_vault_without_losing_exit_policy() {
        let mut fixture = crate::cli::commands::test_support::EmptySession::new();
        let error = search(&mut fixture.session, "needle".into(), true).unwrap_err();
        assert_eq!(error.code, 1);
        assert!(error.message.contains("latch init"), "{}", error.message);
    }
}
