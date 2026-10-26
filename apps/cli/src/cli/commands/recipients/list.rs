use super::super::*;

pub(super) fn list(s: &mut Session, json: bool) -> CliResult {
    let r = s.recipients_list()?.items;
    if json {
        return print_proto_json_list(&r, "RecipientInfo");
    }
    print_recipients(&r);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn propagates_missing_vault_without_losing_exit_policy() {
        let mut fixture = crate::cli::commands::test_support::EmptySession::new();
        let error = list(&mut fixture.session, true).unwrap_err();
        assert_eq!(error.code, 1);
        assert!(error.message.contains("latch init"), "{}", error.message);
    }
}
