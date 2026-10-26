use super::*;

pub(super) fn export(
    s: &mut Session,
    format: &str,
    out: Option<PathBuf>,
    profile: Option<String>,
    yes: bool,
) -> CliResult {
    let (fmt, default_name) = match format.to_ascii_lowercase().as_str() {
        "age" | "latch" | "latch_archive" => (ExportFormat::LatchArchive, "latch-export.latch.age"),
        "json" => (ExportFormat::Json, "latch-export.json"),
        "dotenv" | "env" => (ExportFormat::Dotenv, "latch-export.env"),
        _ => return Err(CliError::user(format!("unknown export format `{format}`"))),
    };
    let path = absolute(&out.unwrap_or_else(|| PathBuf::from(default_name)))?;
    if fmt != ExportFormat::LatchArchive && !yes && !confirm("write secrets to a PLAINTEXT file?") {
        return Err(CliError::user("plaintext export not confirmed (use --yes)"));
    }
    let profile_id = match &profile {
        Some(p) => Some(resolve_profile(s, p)?.id),
        None => None,
    };
    let passphrase = if fmt == ExportFormat::LatchArchive {
        Some(read_passphrase("Archive passphrase: ", true, false)?.to_string())
    } else {
        None
    };
    let r: ExportResult = s.export_run(ExportParams {
        format: fmt as i32,
        path: path.display().to_string(),
        profile_id,
        passphrase,
    })?;
    eprintln!("exported {} entr(ies) to {}", r.entries, r.path);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_path_is_rejected_before_prompting_or_rpc() {
        let mut fixture = crate::cli::commands::test_support::EmptySession::new();
        let error = export(
            &mut fixture.session,
            "age",
            Some(PathBuf::new()),
            Some("missing-profile".into()),
            false,
        )
        .unwrap_err();
        assert_eq!(error.code, 1);
        assert_eq!(error.message, "cannot resolve absolute path");
    }

    #[test]
    fn plaintext_export_requires_confirmation_before_accessing_vault() {
        let mut fixture = crate::cli::commands::test_support::EmptySession::new();
        let error = export(&mut fixture.session, "json", None, None, false).unwrap_err();
        assert_eq!(error.code, 1);
        assert!(error.message.contains("not confirmed"));
    }
}
