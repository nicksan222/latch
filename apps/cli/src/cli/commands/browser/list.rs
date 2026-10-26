use super::*;

pub(super) fn list(s: &mut Session, json: bool) -> CliResult {
    let l = s.browser_list()?.items;
    if json {
        return print_proto_json_list(&l, "BrowserSession");
    }
    let profiles = s.profiles_list().map(|r| r.items).unwrap_or_default();
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
    fn list_json_rejects_launch_options_and_engine_selection() {
        let cli = Cli::try_parse_from(["latch", "browser", "ls", "--json"]).unwrap();
        assert!(
            matches!(cli.cmd, Cmd::Browser(BrowserArgs { cmd: Some(BrowserCmd::Ls { json }), .. }) if json)
        );
        assert!(Cli::try_parse_from(["latch", "browser", "ls", "--engine", "firefox"]).is_err());
    }
}
