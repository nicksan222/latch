use super::*;

fn default_profile_input(
    name: &str,
    color: &str,
    icon: &str,
    description: &str,
    browser: BrowserSettings,
) -> ProfileInput {
    ProfileInput {
        name: name.to_owned(),
        color: color.to_owned(),
        icon: icon.to_owned(),
        description: description.to_owned(),
        pinned: false,
        browser: Some(browser),
    }
}

fn default_browser_settings(s: &mut Session) -> CliResult<BrowserSettings> {
    let response = s.domain_call(DomainRequest {
        operation: Some(domain_request::Operation::DefaultBrowserSettings(Empty {})),
    })?;
    match response.operation {
        Some(domain_response::Operation::DefaultBrowserSettings(settings)) => Ok(settings),
        _ => Err(CliError::new(
            1,
            "unexpected response to default browser settings",
        )),
    }
}

pub(super) fn add(
    s: &mut Session,
    name: String,
    color: String,
    icon: String,
    description: String,
) -> CliResult {
    let browser = default_browser_settings(s)?;
    let p: Profile = s.profiles_create(default_profile_input(
        &name,
        &color,
        &icon,
        &description,
        browser,
    ))?;
    eprintln!("created profile {}", p.name);
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::cli::ProfileCmd;
    use crate::cli::{Cli, Cmd};
    use clap::Parser;
    #[test]
    fn preserves_creation_defaults() {
        let cli = Cli::try_parse_from(["latch", "profile", "add", "Acme"]).unwrap();
        assert!(
            matches!(cli.cmd, Cmd::Profile(ProfileCmd::Add { name, color, icon, description }) if name == "Acme" && color == "#2563eb" && icon == "folder" && description.is_empty())
        );
        assert!(Cli::try_parse_from(["latch", "profile", "add"]).is_err());
    }

    #[test]
    fn new_profiles_start_unpinned_with_generated_managed_browser_defaults() {
        let browser = super::BrowserSettings {
            start_pages: vec![],
            bookmarks: vec![],
            autofill: super::AutofillScope::All as i32,
            autofill_entry_ids: vec![],
        };
        let input = super::default_profile_input("Acme", "#2563eb", "folder", "Client", browser);
        assert_eq!(input.name, "Acme");
        assert_eq!(input.description, "Client");
        assert!(!input.pinned);
        let browser = input.browser.unwrap();
        assert!(browser.start_pages.is_empty());
        assert!(browser.bookmarks.is_empty());
        assert!(browser.autofill_entry_ids.is_empty());
    }
}
