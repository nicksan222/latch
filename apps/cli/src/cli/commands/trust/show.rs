use super::super::*;

pub(super) fn show(s: &mut Session, json: bool) -> CliResult {
    let t: TrustStatus = s.vault_trust()?;
    if json {
        return print_json(&t);
    }
    outln!("fingerprint: {}", t.fingerprint);
    outln!("trust:       {}", trust_state_name(&t));
    outln!("key epoch:   {}", t.key_seq);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn propagates_missing_vault_without_losing_exit_policy() {
        let mut fixture = crate::cli::commands::test_support::EmptySession::new();
        let error = show(&mut fixture.session, true).unwrap_err();
        assert_eq!(error.code, 1);
        assert!(error.message.contains("latch init"), "{}", error.message);
    }
}
