use super::super::*;

pub(super) fn rotate(s: &mut Session) -> CliResult {
    let r: Vec<RecipientInfo> = s.recipients_rotate()?;
    print_recipients(&r);
    eprintln!("vault key rotated");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn propagates_missing_vault_without_losing_exit_policy() {
        let mut fixture = crate::cli::commands::test_support::EmptySession::new();
        let error = rotate(&mut fixture.session).unwrap_err();
        assert_eq!(error.code, 1);
        assert!(error.message.contains("latch init"), "{}", error.message);
    }
}
