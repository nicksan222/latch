use super::super::*;

pub(super) fn uninstall(s: &mut Session, path: Option<PathBuf>) -> CliResult {
    let root = match path {
        Some(p) => absolute(&p)?,
        None => local_vault_root(s)?,
    };
    let result = s.hooks_uninstall(HookParams {
        path: root.display().to_string(),
        executable: None,
    })?;
    if result.removed {
        eprintln!("removed pre-commit hook");
    } else {
        eprintln!("no Latch pre-commit hook installed");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_path_is_rejected_before_opening_a_vault() {
        let mut fixture = crate::cli::commands::test_support::EmptySession::new();
        let error = uninstall(&mut fixture.session, Some(PathBuf::new())).unwrap_err();
        assert_eq!(error.code, 1);
        assert_eq!(error.message, "cannot resolve absolute path");
    }

    #[test]
    fn rejects_a_non_vault_without_writing_hooks() {
        let mut fixture = crate::cli::commands::test_support::EmptySession::new();
        let directory = tempfile::tempdir().unwrap();
        let error = uninstall(&mut fixture.session, Some(directory.path().to_owned())).unwrap_err();
        assert_eq!(error.code, 1);
        assert!(!directory.path().join(".git/hooks/pre-commit").exists());
    }
}
