use super::*;

fn default_profile_input(name: &str, color: &str, icon: &str, description: &str) -> ProfileInput {
    ProfileInput {
        name: name.to_owned(),
        color: color.to_owned(),
        icon: icon.to_owned(),
        description: description.to_owned(),
        pinned: false,
        browser: BrowserSettings {
            engine: BrowserEngine::Chromium,
            binary: None,
            start_pages: vec![],
            bookmarks: vec![],
            extensions_allowed: false,
            autofill: AutofillScope::All,
            autofill_entry_ids: vec![],
        },
    }
}

pub(super) fn add(
    s: &mut Session,
    name: String,
    color: String,
    icon: String,
    description: String,
) -> CliResult {
    let p: Profile =
        s.profiles_create(default_profile_input(&name, &color, &icon, &description))?;
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
    fn new_profiles_start_unpinned_without_browser_extensions_or_session_configuration() {
        let input = super::default_profile_input("Acme", "#2563eb", "folder", "Client");
        assert_eq!(input.name, "Acme");
        assert_eq!(input.description, "Client");
        assert!(!input.pinned);
        assert_eq!(input.browser.engine, super::BrowserEngine::Chromium);
        assert!(input.browser.binary.is_none());
        assert!(input.browser.start_pages.is_empty());
        assert!(input.browser.bookmarks.is_empty());
        assert!(!input.browser.extensions_allowed);
        assert!(input.browser.autofill_entry_ids.is_empty());
    }
}
