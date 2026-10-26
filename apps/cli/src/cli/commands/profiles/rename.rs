use super::*;

fn profile_to_input(p: &Profile) -> ProfileInput {
    ProfileInput {
        name: p.name.clone(),
        color: p.color.clone(),
        icon: p.icon.clone(),
        description: p.description.clone(),
        pinned: p.pinned,
        browser: p.browser.clone(),
    }
}

pub(super) fn rename(s: &mut Session, old: String, new: String) -> CliResult {
    let sum = resolve_profile(s, &old)?;
    let p: Profile = s.profiles_get(IdParams { id: sum.id.clone() })?;
    let mut input = profile_to_input(&p);
    input.name = new;
    let p: Profile = s.profiles_update(ProfileUpdateParams { id: sum.id, input })?;
    eprintln!("renamed to {}", p.name);
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::cli::ProfileCmd;
    use crate::cli::{Cli, Cmd};
    use clap::Parser;
    #[test]
    fn requires_both_names() {
        let cli = Cli::try_parse_from(["latch", "profile", "rename", "Acme", "New name"]).unwrap();
        assert!(
            matches!(cli.cmd, Cmd::Profile(ProfileCmd::Rename { old, new }) if old == "Acme" && new == "New name")
        );
        assert!(Cli::try_parse_from(["latch", "profile", "rename", "Acme"]).is_err());
    }

    #[test]
    fn rename_input_preserves_metadata_and_browser_settings() {
        let profile: super::Profile = serde_json::from_value(serde_json::json!({
            "id": "profile-id", "name": "Acme", "color": "#123456", "icon": "briefcase",
            "description": "Client project", "pinned": true,
            "createdAt": "created", "updatedAt": "updated",
            "browser": {
                "engine": "firefox", "binary": "/usr/bin/firefox",
                "startPages": ["https://example.com"],
                "bookmarks": [{ "title": "Console", "url": "https://example.com/console" }],
                "extensionsAllowed": true, "autofill": "selected", "autofillEntryIds": ["entry-id"]
            }
        }))
        .unwrap();
        let mut input = super::profile_to_input(&profile);
        input.name = "New name".into();
        assert_eq!(input.name, "New name");
        assert_eq!(input.color, profile.color);
        assert_eq!(input.icon, profile.icon);
        assert_eq!(input.description, profile.description);
        assert_eq!(input.pinned, profile.pinned);
        assert_eq!(
            serde_json::to_value(input.browser).unwrap(),
            serde_json::to_value(profile.browser).unwrap()
        );
    }
}
