use super::*;

pub(super) fn list(s: &mut Session, json: bool) -> CliResult {
    let list = s.profiles_list()?.items;
    if json {
        return print_proto_json_list(&list, "ProfileSummary");
    }
    for p in list {
        outln!(
            "{}{}  {} entries{}",
            p.name,
            if p.pinned { " *" } else { "" },
            p.entry_count,
            if p.browser_running {
                "  (browser running)"
            } else {
                ""
            }
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::cli::ProfileCmd;
    use crate::cli::{Cli, Cmd};
    use clap::Parser;
    #[test]
    fn json_flag_belongs_to_list() {
        let cli = Cli::try_parse_from(["latch", "profile", "ls", "--json"]).unwrap();
        assert!(matches!(cli.cmd, Cmd::Profile(ProfileCmd::Ls { json }) if json));
        assert!(Cli::try_parse_from(["latch", "profile", "ls", "unexpected"]).is_err());
    }
}
