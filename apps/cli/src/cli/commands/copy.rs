use super::*;

pub(super) fn copy(
    s: &mut Session,
    target: String,
    field: Option<String>,
    clear: Option<String>,
) -> CliResult {
    let (_, e) = resolve_entry(s, &target)?;
    let field = match field {
        Some(f) => f,
        None => default_field(EntryType::try_from(e.r#type).unwrap_or(EntryType::Unspecified))?
            .to_owned(),
    };
    let p = ClipboardCopyParams {
        id: e.id,
        field,
        clear_after_secs: clear.as_deref().map(parse_duration).transpose()?,
    };
    let r: ClipboardCopyResult = s.clipboard_copy(p)?;
    copied(s, r.clear_after_secs)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn propagates_missing_vault_without_losing_exit_policy() {
        let mut fixture = crate::cli::commands::test_support::EmptySession::new();
        let error = copy(&mut fixture.session, "work/key".into(), None, None).unwrap_err();
        assert_eq!(error.code, 1);
        assert!(error.message.contains("latch init"), "{}", error.message);
    }
}
