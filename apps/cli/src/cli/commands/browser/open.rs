use super::*;

pub(super) fn open(s: &mut Session, a: BrowserArgs) -> CliResult {
    let Some(name) = a.profile else {
        return Err(CliError::user(
            "usage: latch browser <profile> | ls | close <profile> | reset <profile>",
        ));
    };
    let p = resolve_profile(s, &name)?;
    eprintln!("preparing managed Firefox for {}…", p.name);
    let b: BrowserSession = s.browser_launch(BrowserLaunchParams {
        profile_id: p.id,
        url: a.url,
    })?;
    eprintln!("launched Firefox for {} (pid {})", p.name, b.pid);
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::cli::{Cli, Cmd};
    use clap::Parser;
    #[test]
    fn browser_open_uses_managed_firefox_without_an_engine_selector() {
        assert!(Cli::try_parse_from(["latch", "browser", "Acme", "--engine", "firefox"]).is_err());
        let cli = Cli::try_parse_from(["latch", "browser", "Acme", "--url", "https://example.com"])
            .unwrap();
        assert!(
            matches!(cli.cmd, Cmd::Browser(a) if a.cmd.is_none() && a.profile.as_deref() == Some("Acme") && a.url.as_deref() == Some("https://example.com"))
        );
    }
}
