use super::*;

pub(super) fn trash(s: &mut Session, profile: Option<String>, json: bool) -> CliResult {
    let pid = match &profile {
        Some(p) => Some(resolve_profile(s, p)?.id),
        None => None,
    };
    let t = s.entries_trash(TrashParams { profile_id: pid })?.items;
    if json {
        return print_proto_json_list(&t, "TrashItem");
    }
    for i in t {
        let short: String = i.commit.chars().take(10).collect();
        outln!(
            "{}  {}  {}  deleted {}  (restore at {short})",
            i.id,
            type_name_code(i.r#type),
            i.name,
            i.deleted_at
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_vault_is_reported_instead_of_an_empty_trash() {
        let mut fixture = crate::cli::commands::test_support::EmptySession::new();
        let error = trash(&mut fixture.session, None, true).unwrap_err();
        assert_eq!(error.code, 1);
        assert!(error.message.contains("latch init"));
    }
}
