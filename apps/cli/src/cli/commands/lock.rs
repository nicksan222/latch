use super::*;

pub(super) fn lock(s: &mut Session) -> CliResult {
    let _: VaultStatus = s.vault_lock()?;
    eprintln!("locked");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn locking_an_uninitialized_agent_is_idempotent() {
        let mut fixture = crate::cli::commands::test_support::EmptySession::new();
        lock(&mut fixture.session).unwrap();
        lock(&mut fixture.session).unwrap();
        let status: VaultStatus = fixture.session.vault_status().unwrap();
        assert!(status.locked);
        assert!(!status.initialized);
    }
}
