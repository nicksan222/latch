use super::*;

fn browser_description(browser: Option<&BrowserSettings>) -> &'static str {
    if browser.is_some() {
        "managed Firefox"
    } else {
        "not configured"
    }
}

pub(super) fn show(s: &mut Session, name: String, json: bool) -> CliResult {
    let sum = resolve_profile(s, &name)?;
    let p: Profile = s.profiles_get(IdParams { id: sum.id })?;
    if json {
        return print_proto_json(&p, "Profile");
    }
    outln!("name:        {}", p.name);
    outln!("id:          {}", p.id);
    outln!("color:       {}", p.color);
    outln!("icon:        {}", p.icon);
    outln!("description: {}", p.description);
    outln!("entries:     {}", sum.entry_count);
    outln!("browser:     {}", browser_description(p.browser.as_ref()));
    outln!("created:     {}", p.created_at);
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::cli::ProfileCmd;
    use crate::cli::{Cli, Cmd};
    use clap::Parser;
    #[test]
    fn requires_name_with_optional_json() {
        let cli = Cli::try_parse_from(["latch", "profile", "show", "Acme", "--json"]).unwrap();
        assert!(
            matches!(cli.cmd, Cmd::Profile(ProfileCmd::Show { name, json }) if name == "Acme" && json)
        );
        assert!(Cli::try_parse_from(["latch", "profile", "show", "--json"]).is_err());
    }

    #[test]
    fn describes_configured_profiles_as_managed_firefox_without_engine_selection() {
        assert_eq!(
            super::browser_description(Some(&super::BrowserSettings::default())),
            "managed Firefox"
        );
        assert_eq!(super::browser_description(None), "not configured");
    }
}
