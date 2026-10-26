use super::*;

pub(super) fn totp(s: &mut Session, target: String, copy: bool) -> CliResult {
    let (_, e) = resolve_entry(s, &target)?;
    if copy {
        let r: ClipboardCopyResult = s.clipboard_copy(ClipboardCopyParams {
            id: e.id,
            field: "totp".into(),
            clear_after_secs: None,
        })?;
        return copied(s, r.clear_after_secs);
    }
    let c: TotpCode = s.totp_get(IdParams { id: e.id })?;
    outln!("{}", c.code);
    if std::io::stderr().is_terminal() {
        eprintln!("expires in {} s", c.expires_in);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn propagates_missing_vault_without_losing_exit_policy() {
        let mut fixture = crate::cli::commands::test_support::EmptySession::new();
        let error = totp(&mut fixture.session, "work/key".into(), false).unwrap_err();
        assert_eq!(error.code, 1);
        assert!(error.message.contains("latch init"), "{}", error.message);
    }
}
