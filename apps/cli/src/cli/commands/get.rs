use super::*;

pub(super) fn get(
    s: &mut Session,
    target: &str,
    field: Option<String>,
    json: bool,
    reveal: bool,
    yes: bool,
) -> CliResult {
    // Guard before both branches: printing a single field is still a secret.
    if reveal && !yes && !stdout_tty() {
        return Err(CliError::user(
            "refusing to print secrets to a non-terminal; add --yes to confirm",
        ));
    }
    let (p, e) = resolve_entry(s, target)?;
    if let Some(f) = field {
        if !reveal && !is_public_field(&f) {
            outln!("{MASK}");
            eprintln!("(masked; add --reveal to print `{f}`)");
            return Ok(());
        }
        let r: RevealResult = s.entries_reveal(RevealParams { id: e.id, field: f })?;
        let v = Zeroizing::new(r.value);
        outln!("{}", v.as_str());
        return Ok(());
    }
    let entry = get_entry(s, &e.id, reveal)?;
    if json {
        return print_json(&entry);
    }
    print_entry(&entry, &p.name);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn non_terminal_reveal_requires_confirmation_before_resolving_entry() {
        let mut fixture = crate::cli::commands::test_support::EmptySession::new();
        let error = get(&mut fixture.session, "work/key", None, false, true, false).unwrap_err();
        assert_eq!(error.code, 1);
        assert!(error.message.contains("--yes"));
    }
}
