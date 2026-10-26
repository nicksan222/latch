use super::super::*;

pub(super) fn remove(s: &mut Session, label: String) -> CliResult {
    let r = s.recipients_remove(RemoveRecipientParams { label })?.items;
    print_recipients(&r);
    eprintln!(
        "note: the removed key can still read existing history; run `latch recipients rotate`"
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn propagates_missing_vault_without_losing_exit_policy() {
        let mut fixture = crate::cli::commands::test_support::EmptySession::new();
        let error = remove(&mut fixture.session, "device".into()).unwrap_err();
        assert_eq!(error.code, 1);
        assert!(error.message.contains("latch init"), "{}", error.message);
    }
}
