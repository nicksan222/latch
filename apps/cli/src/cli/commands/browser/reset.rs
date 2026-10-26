use super::*;

pub(super) fn reset(s: &mut Session, profile: String) -> CliResult {
    let p = resolve_profile(s, &profile)?;
    s.browser_reset(IdParams { id: p.id })?;
    eprintln!("discarded browser session of {}", p.name);
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::cli::{BrowserArgs, BrowserCmd};
    use crate::cli::{Cli, Cmd};
    use clap::Parser;
    #[test]
    fn requires_profile() {
        let cli = Cli::try_parse_from(["latch", "browser", "reset", "Acme"]).unwrap();
        assert!(
            matches!(cli.cmd, Cmd::Browser(BrowserArgs { cmd: Some(BrowserCmd::Reset { profile }), .. }) if profile == "Acme")
        );
        assert!(Cli::try_parse_from(["latch", "browser", "reset"]).is_err());
    }
}
