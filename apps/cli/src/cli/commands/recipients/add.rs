use super::super::*;

pub(super) fn add(s: &mut Session, public_key: String, label: String) -> CliResult {
    let r: Vec<RecipientInfo> = s.recipients_add(AddRecipientParams { label, public_key })?;
    print_recipients(&r);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn propagates_missing_vault_without_losing_exit_policy() {
        let mut fixture = crate::cli::commands::test_support::EmptySession::new();
        let error = add(
            &mut fixture.session,
            "invalid-age-key".into(),
            "device".into(),
        )
        .unwrap_err();
        assert_eq!(error.code, 1);
        assert!(error.message.contains("latch init"), "{}", error.message);
    }
}
