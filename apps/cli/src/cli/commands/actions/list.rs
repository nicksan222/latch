use super::*;

pub(super) fn list(s: &mut Session, profile: String) -> CliResult {
    let p = resolve_profile(s, &profile)?;
    let list: ActionListResult = s.actions_list(IdParams { id: p.id })?;
    for a in list.actions {
        outln!("{}  {}", a.id, a.name);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::cli::ActionCmd;
    use crate::cli::{Cli, Cmd};
    use clap::Parser;
    #[test]
    fn requires_profile() {
        let cli = Cli::try_parse_from(["latch", "action", "ls", "Acme"]).unwrap();
        assert!(matches!(cli.cmd, Cmd::Action(ActionCmd::Ls { profile }) if profile == "Acme"));
        assert!(Cli::try_parse_from(["latch", "action", "ls"]).is_err());
    }
}
