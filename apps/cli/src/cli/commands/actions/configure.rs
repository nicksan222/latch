use super::*;

pub(super) fn configure(s: &mut Session, profile: String, path: String) -> CliResult {
    let p = resolve_profile(s, &profile)?;
    s.actions_configure(ActionConfigureParams {
        profile_id: p.id,
        working_directory: if path.is_empty() { None } else { Some(path) },
    })?;
    eprintln!("local project directory updated");
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::cli::ActionCmd;
    use crate::cli::{Cli, Cmd};
    use clap::Parser;
    #[test]
    fn empty_directory_is_explicit_clear() {
        let cli = Cli::try_parse_from(["latch", "action", "directory", "Acme", ""]).unwrap();
        assert!(
            matches!(cli.cmd, Cmd::Action(ActionCmd::Directory { profile, path }) if profile == "Acme" && path.is_empty())
        );
        assert!(Cli::try_parse_from(["latch", "action", "directory", "Acme"]).is_err());
    }
}
