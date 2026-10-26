use super::*;

pub(super) fn remove(s: &mut Session, name: String, yes: bool) -> CliResult {
    let p = resolve_profile(s, &name)?;
    if !yes
        && !confirm(&format!(
            "delete profile `{}` and its {} entries?",
            p.name, p.entry_count
        ))
    {
        return Err(CliError::user("not confirmed (use --yes)"));
    }
    s.profiles_delete(IdParams { id: p.id })?;
    eprintln!("deleted profile {}", p.name);
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::cli::ProfileCmd;
    use crate::cli::{Cli, Cmd};
    use clap::Parser;
    #[test]
    fn accepts_short_confirmation_flag() {
        let cli = Cli::try_parse_from(["latch", "profile", "rm", "Acme", "-y"]).unwrap();
        assert!(
            matches!(cli.cmd, Cmd::Profile(ProfileCmd::Rm { name, yes }) if name == "Acme" && yes)
        );
        assert!(Cli::try_parse_from(["latch", "profile", "rm"]).is_err());
    }
}
