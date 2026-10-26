use super::*;

pub(super) fn unlock(
    s: &mut Session,
    timeout: Option<String>,
    identity: Option<PathBuf>,
    passphrase_stdin: bool,
) -> CliResult {
    let timeout_secs = timeout.as_deref().map(parse_lock_timeout).transpose()?;
    let params = match identity {
        Some(f) => UnlockParams {
            passphrase: None,
            identity_file: Some(f.display().to_string()),
            timeout_secs,
        },
        None => {
            if let Some(id) = std::env::var(session::ENV_IDENTITY_FILE)
                .ok()
                .filter(|v| !v.is_empty())
            {
                UnlockParams {
                    passphrase: None,
                    identity_file: Some(id),
                    timeout_secs,
                }
            } else {
                let pw = read_passphrase("Vault passphrase: ", false, passphrase_stdin)?;
                UnlockParams {
                    passphrase: Some(pw.to_string()),
                    identity_file: None,
                    timeout_secs,
                }
            }
        }
    };
    let st: VaultStatus = s.vault_unlock(params)?;
    eprintln!("unlocked{}", locks_in(&st));
    trust_notice(&st);
    integrity_probe(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_timeout_is_rejected_before_prompting() {
        let mut fixture = crate::cli::commands::test_support::EmptySession::new();
        let error = unlock(
            &mut fixture.session,
            Some("invalid-timeout".into()),
            None,
            false,
        )
        .unwrap_err();
        assert_eq!(error.code, 1);
        assert!(error.message.contains("duration"));
    }
}
