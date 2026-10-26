use super::*;

fn default_vault_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("latch-vault")
}

pub(super) fn init(s: &mut Session, a: InitArgs) -> CliResult {
    let path = absolute(&a.path.clone().unwrap_or_else(default_vault_path))?;
    if let Some(url) = &a.clone {
        let st: VaultStatus = s.vault_clone(CloneParams {
            url: url.clone(),
            path: path.display().to_string(),
        })?;
        eprintln!(
            "cloned vault {} into {} (locked; run `latch unlock`)",
            st.vault_id.unwrap_or_default(),
            path.display()
        );
        return Ok(());
    }
    let pw = read_passphrase("New vault passphrase: ", true, a.passphrase_stdin)?;
    // Weak KDF only under the explicit test escape hatch.
    let kdf = crypto::weak_kdf_allowed().then_some(KdfParams {
        memory_kib: crypto::WEAK_KDF.memory_kib,
        iterations: crypto::WEAK_KDF.iterations,
        parallelism: crypto::WEAK_KDF.parallelism,
    });
    if kdf.is_none() {
        eprintln!("calibrating key derivation (about 1 s)…");
    }
    let r: InitResult = s.vault_init(InitParams {
        path: path.display().to_string(),
        passphrase: pw.to_string(),
        kdf,
        git_name: a.git_name,
        git_email: a.git_email,
        remote_url: a.remote,
        recovery_kit: a.recovery_kit,
    })?;
    eprintln!(
        "created vault {} at {}",
        r.status
            .as_ref()
            .and_then(|st| st.vault_id.clone())
            .unwrap_or_default(),
        path.display()
    );
    if let Some(kit) = r.recovery_kit {
        let doc = Zeroizing::new(kit.document);
        outln!("{}", doc.as_str());
        eprintln!("store the recovery kit above offline; Latch does not keep a copy");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_path_is_rejected_before_prompting_or_rpc() {
        let mut fixture = crate::cli::commands::test_support::EmptySession::new();
        let error = init(
            &mut fixture.session,
            InitArgs {
                path: Some(PathBuf::new()),
                clone: None,
                remote: None,
                recovery_kit: false,
                git_name: None,
                git_email: None,
                passphrase_stdin: false,
            },
        )
        .unwrap_err();
        assert_eq!(error.code, 1);
        assert_eq!(error.message, "cannot resolve absolute path");
    }

    #[test]
    fn absolute_paths_are_preserved_for_vault_creation() {
        let directory = tempfile::tempdir().unwrap();
        assert_eq!(absolute(directory.path()).unwrap(), directory.path());
    }

    #[test]
    fn clone_failure_does_not_create_an_initialized_vault() {
        let mut fixture = crate::cli::commands::test_support::EmptySession::new();
        let directory = tempfile::tempdir().unwrap();
        let error = init(
            &mut fixture.session,
            InitArgs {
                path: Some(directory.path().join("vault")),
                clone: Some(directory.path().join("missing.git").display().to_string()),
                remote: None,
                recovery_kit: false,
                git_name: None,
                git_email: None,
                passphrase_stdin: false,
            },
        )
        .unwrap_err();
        assert_eq!(error.code, 1);
        let state: VaultStatus = fixture.session.vault_status().unwrap();
        assert!(!state.initialized);
    }
}
