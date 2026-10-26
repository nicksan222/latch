use super::*;

pub(super) fn close(s: &mut Session, profile: String) -> CliResult {
    let p = resolve_profile(s, &profile)?;
    s.browser_close(IdParams { id: p.id })?;
    eprintln!("closed browser for {}", p.name);
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::cli::{BrowserArgs, BrowserCmd};
    use crate::cli::{Cli, Cmd};
    use clap::Parser;
    #[test]
    fn requires_profile() {
        let cli = Cli::try_parse_from(["latch", "browser", "close", "Acme"]).unwrap();
        assert!(
            matches!(cli.cmd, Cmd::Browser(BrowserArgs { cmd: Some(BrowserCmd::Close { profile }), .. }) if profile == "Acme")
        );
        assert!(Cli::try_parse_from(["latch", "browser", "close"]).is_err());
    }
}
