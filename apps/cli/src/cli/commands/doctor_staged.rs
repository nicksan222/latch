use super::*;

pub(super) fn doctor_staged(s: &mut Session, json: bool) -> CliResult {
    let root = local_vault_root(s)?;
    scan_staged(s, &root, json)
}

fn scan_staged(s: &mut Session, root: &Path, json: bool) -> CliResult {
    let r = s.doctor_scan(OpenParams {
        path: root.display().to_string(),
    })?;
    print_doctor(&r, json)?;
    if !r.ok {
        return Err(CliError::user(
            "commit rejected: staged files failed the plaintext scan",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn staged_scan_rejects_non_vault_without_creating_files() {
        let mut fixture = crate::cli::commands::test_support::EmptySession::new();
        let directory = tempfile::tempdir().unwrap();
        let error = scan_staged(&mut fixture.session, directory.path(), true).unwrap_err();
        assert_eq!(error.code, 1);
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
    }
}
