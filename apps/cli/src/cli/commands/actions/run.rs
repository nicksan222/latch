use super::*;

pub(super) fn run(s: &mut Session, profile: String, action: String) -> CliResult {
    let p = resolve_profile(s, &profile)?;
    let list: ActionListResult = s.actions_list(IdParams { id: p.id.clone() })?;
    let found = if let Some(a) = list.actions.iter().find(|a| a.id == action) {
        a
    } else {
        let mut names = list
            .actions
            .iter()
            .filter(|a| a.name.eq_ignore_ascii_case(&action));
        let a = names
            .next()
            .ok_or_else(|| CliError::user("saved action not found"))?;
        if names.next().is_some() {
            return Err(CliError::user(
                "multiple actions have this name; use the action ID",
            ));
        }
        a
    };
    s.actions_run(ActionIdParams {
        profile_id: p.id,
        action_id: found.id.clone(),
    })?;
    eprintln!("action launched");
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::cli::ActionCmd;
    use crate::cli::{Cli, Cmd};
    use clap::Parser;
    #[test]
    fn requires_profile_and_preserves_action_name() {
        let cli =
            Cli::try_parse_from(["latch", "action", "run", "Acme", "Start development"]).unwrap();
        assert!(
            matches!(cli.cmd, Cmd::Action(ActionCmd::Run { profile, action }) if profile == "Acme" && action == "Start development")
        );
        assert!(Cli::try_parse_from(["latch", "action", "run", "Acme"]).is_err());
    }
}
