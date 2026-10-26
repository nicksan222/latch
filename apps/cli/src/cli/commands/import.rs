use super::*;

fn parse_import_format(f: &str) -> CliResult<ImportFormat> {
    Ok(match f.to_ascii_lowercase().replace('-', "_").as_str() {
        "dotenv" | "env" => ImportFormat::Dotenv,
        "bitwarden" => ImportFormat::Bitwarden,
        "1password" | "onepassword" | "one_password_csv" | "1password_csv" => {
            ImportFormat::OnePasswordCsv
        }
        "keepass" | "keepass_xml" => ImportFormat::KeepassXml,
        "browser" | "browser_csv" | "chrome" | "firefox" => ImportFormat::BrowserCsv,
        "pass" => ImportFormat::Pass,
        "json" => ImportFormat::Json,
        "yaml" | "yml" => ImportFormat::Yaml,
        "latch" | "latch_archive" | "age" => ImportFormat::LatchArchive,
        _ => return Err(CliError::user(format!("unknown import format `{f}`"))),
    })
}

pub(super) fn import(
    s: &mut Session,
    format: &str,
    file: &Path,
    profile: Option<String>,
    dry_run: bool,
    skip_duplicates: bool,
) -> CliResult {
    let format = parse_import_format(format)?;
    let path = absolute(file)?;
    let profile_id = match &profile {
        Some(p) => Some(resolve_profile(s, p)?.id),
        None => None,
    };
    let passphrase = if format == ImportFormat::LatchArchive {
        Some(read_passphrase("Archive passphrase: ", false, false)?.to_string())
    } else {
        None
    };
    let params = ImportParams {
        format: format as i32,
        path: path.display().to_string(),
        profile_id,
        skip_duplicates,
        passphrase,
    };
    if dry_run {
        let p: ImportPreview = s.import_preview(params)?;
        for i in &p.items {
            outln!(
                "{}/{}  [{}]{}",
                i.profile_name,
                i.name,
                type_name(EntryType::try_from(i.r#type).unwrap_or(EntryType::Unspecified)),
                if i.duplicate { "  (duplicate)" } else { "" }
            );
        }
        for w in &p.warnings {
            eprintln!("warning: {w}");
        }
        eprintln!("{} item(s); nothing imported (dry run)", p.items.len());
        return Ok(());
    }
    let r: ImportApplyResult = s.import_apply(params)?;
    eprintln!(
        "imported {} entr(ies), skipped {}, created {} profile(s)",
        r.created, r.skipped, r.profiles_created
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_path_is_rejected_before_prompting_or_rpc() {
        let mut fixture = crate::cli::commands::test_support::EmptySession::new();
        let error = import(
            &mut fixture.session,
            "age",
            Path::new(""),
            Some("missing-profile".into()),
            false,
            false,
        )
        .unwrap_err();
        assert_eq!(error.code, 1);
        assert_eq!(error.message, "cannot resolve absolute path");
    }

    #[test]
    fn supported_format_aliases_remain_compatible() {
        for name in ["1password", "1password-csv", "ONEPASSWORD"] {
            assert_eq!(
                parse_import_format(name).unwrap(),
                ImportFormat::OnePasswordCsv
            );
        }
        assert_eq!(parse_import_format("yml").unwrap(), ImportFormat::Yaml);
        assert_eq!(
            parse_import_format("firefox").unwrap(),
            ImportFormat::BrowserCsv
        );
        assert_eq!(
            parse_import_format("age").unwrap(),
            ImportFormat::LatchArchive
        );
    }

    #[test]
    fn unknown_format_is_rejected_before_reading_file_or_prompting() {
        let mut fixture = crate::cli::commands::test_support::EmptySession::new();
        let error = import(
            &mut fixture.session,
            "unsupported",
            Path::new("missing"),
            None,
            false,
            false,
        )
        .unwrap_err();
        assert_eq!(error.code, 1);
        assert!(error.message.contains("unknown import format"));
    }
}
