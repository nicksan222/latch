use super::*;

pub(super) fn remove(s: &mut Session, target: String) -> CliResult {
    let (p, e) = resolve_entry(s, &target)?;
    s.entries_delete(IdParams { id: e.id })?;
    eprintln!("deleted {}/{} (see `latch trash`)", p.name, e.name);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn propagates_missing_vault_without_losing_exit_policy() {
        let mut fixture = crate::cli::commands::test_support::EmptySession::new();
        let error = remove(&mut fixture.session, "work/key".into()).unwrap_err();
        assert_eq!(error.code, 1);
        assert!(error.message.contains("latch init"), "{}", error.message);
    }
}
