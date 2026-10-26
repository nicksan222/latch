use super::*;

pub(super) fn history(s: &mut Session, target: String, json: bool) -> CliResult {
    let (_, e) = resolve_entry(s, &target)?;
    let h: Vec<HistoryItem> = s.entries_history(IdParams { id: e.id })?;
    if json {
        return print_json(&h);
    }
    for i in h {
        let short: String = i.commit.chars().take(10).collect();
        let fields = if i.deleted {
            "deleted".to_owned()
        } else {
            i.changed_fields.join(", ")
        };
        outln!("{short}  {}  {}  {}  [{fields}]", i.at, i.author, i.message);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn propagates_missing_vault_without_losing_exit_policy() {
        let mut fixture = crate::cli::commands::test_support::EmptySession::new();
        let error = history(&mut fixture.session, "work/key".into(), true).unwrap_err();
        assert_eq!(error.code, 1);
        assert!(error.message.contains("latch init"), "{}", error.message);
    }
}
