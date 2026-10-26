use super::*;

pub(super) fn doctor(s: &mut Session, fix: bool, rebuild_manifest: bool, json: bool) -> CliResult {
    let r: DoctorReport = s.doctor_run(DoctorParams {
        fix,
        rebuild_manifest,
        staged: false,
    })?;
    print_doctor(&r, json)?;
    if r.issues
        .iter()
        .any(|i| Severity::try_from(i.severity) == Ok(Severity::Error))
    {
        return Err(CliError::new(3, ""));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_vault_is_not_reported_as_healthy() {
        let mut fixture = crate::cli::commands::test_support::EmptySession::new();
        let error = doctor(&mut fixture.session, false, false, true).unwrap_err();
        assert_eq!(error.code, 1);
        assert!(error.message.contains("latch init"));
        let state: VaultStatus = fixture.session.vault_status().unwrap();
        assert!(!state.initialized);
    }
}
