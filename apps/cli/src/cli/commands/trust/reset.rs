use super::super::*;

pub(super) fn reset(s: &mut Session, yes: bool) -> CliResult {
    let t: TrustStatus = s.vault_forget_trust(ForgetTrustParams { confirm: yes })?;
    eprintln!(
        "trust reset; the next unlock trusts the repository's keys (fingerprint now {})",
        t.fingerprint
    );
    eprintln!("verify that fingerprint on another device before unlocking");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn propagates_missing_vault_without_losing_exit_policy() {
        let mut fixture = crate::cli::commands::test_support::EmptySession::new();
        let error = reset(&mut fixture.session, false).unwrap_err();
        assert_eq!(error.code, 1);
        assert!(error.message.contains("latch init"), "{}", error.message);
    }
}
