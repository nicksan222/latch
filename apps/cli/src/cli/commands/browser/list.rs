use super::*;

pub(super) fn list(s: &mut Session, json: bool) -> CliResult {
    let l: Vec<BrowserSession> = s.browser_list()?;
    if json {
        return print_json(&l);
    }
    let profiles: Vec<ProfileSummary> = s.profiles_list().unwrap_or_default();
    for b in l {
        let name = profiles
            .iter()
            .find(|p| p.id == b.profile_id)
            .map(|p| p.name.clone())
            .unwrap_or(b.profile_id.clone());
        outln!("{name}  pid {}  since {}", b.pid, b.started_at);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::cli::{BrowserArgs, BrowserCmd};
    use crate::cli::{Cli, Cmd};
    use clap::Parser;
    #[test]
    fn list_json_rejects_launch_options() {
        let cli = Cli::try_parse_from(["latch", "browser", "ls", "--json"]).unwrap();
        assert!(
            matches!(cli.cmd, Cmd::Browser(BrowserArgs { cmd: Some(BrowserCmd::Ls { json }), .. }) if json)
        );
        assert!(Cli::try_parse_from(["latch", "browser", "ls", "--engine", "firefox"]).is_err());
    }
}
