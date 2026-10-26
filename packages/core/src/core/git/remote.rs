//! Origin configuration and guarded remote transport.

use std::path::Path;

use super::{Error, Git, Result, base_command, git_err};

/// Reject remote URLs that would store a secret in `.git/config` (a password
/// in the user info, e.g. `https://user:token@host/repo`, or a token used as
/// an HTTP username) or could be parsed as an option. Use a credential helper
/// or SSH keys instead.
pub fn check_remote_url(url: &str) -> Result<()> {
    let u = url.trim();
    if u.starts_with('-') || u.chars().any(char::is_control) {
        return Err(Error::InvalidParams("invalid remote URL".into()));
    }
    if let Ok(parsed) = url::Url::parse(u)
        && (parsed.password().is_some()
            || (matches!(parsed.scheme(), "http" | "https") && !parsed.username().is_empty()))
    {
        return Err(Error::InvalidParams(
            "remote URLs must not contain a password or token; use a git credential helper or SSH"
                .into(),
        ));
    }
    Ok(())
}

/// A remote URL safe to display or serialize: any user info of a URL with a
/// scheme is replaced by `***` (http(s) usernames are often tokens).
pub fn redact_url(url: &str) -> String {
    match url::Url::parse(url) {
        Ok(mut u) if !u.username().is_empty() || u.password().is_some() => {
            let keep_user =
                matches!(u.scheme(), "ssh" | "git+ssh" | "ssh+git") && u.password().is_none();
            if !keep_user {
                let _ = u.set_password(None);
                let _ = u.set_username("***");
            }
            u.to_string()
        }
        _ => url.to_owned(),
    }
}

/// `git clone <url> <path>`.
pub fn clone(url: &str, path: &Path) -> Result<Git> {
    check_remote_url(url)?;
    let p = path.to_string_lossy().to_string();
    let args = ["clone", "--quiet", "--", url, p.as_str()];
    let out = base_command()
        .args(args)
        .output()
        .map_err(|e| Error::Git(format!("cannot run git: {e}")))?;
    if !out.status.success() {
        return Err(git_err(&args, &out));
    }
    Ok(Git::new(path))
}

impl Git {
    pub fn remote_url(&self) -> Option<String> {
        self.config_get("remote.origin.url")
    }

    pub fn set_remote(&self, url: &str) -> Result<()> {
        let has = self.remote_url().is_some();
        if !url.trim().is_empty() {
            check_remote_url(url)?;
        }
        if url.trim().is_empty() {
            if has {
                self.run(&["remote", "remove", "origin"])?;
            }
            return Ok(());
        }
        if has {
            self.run(&["remote", "set-url", "origin", url])?;
        } else {
            self.run(&["remote", "add", "origin", url])?;
        }
        Ok(())
    }

    // Validate existing configurations as well as newly configured remotes.
    // In particular, never let Git echo a legacy credential URL in stderr.
    fn check_origin_url(&self) -> Result<()> {
        // Both keys can have multiple values. `config --get` returns only the
        // last one, whereas Git may transport to earlier URLs as well.
        for key in ["remote.origin.url", "remote.origin.pushurl"] {
            let args = ["config", "--null", "--get-all", key];
            let out = self.output(&args)?;
            match out.status.code() {
                Some(0) => {
                    for url in String::from_utf8_lossy(&out.stdout)
                        .split('\0')
                        .filter(|url| !url.is_empty())
                    {
                        check_remote_url(url)?;
                    }
                }
                Some(1) => {} // No configured value.
                _ => return Err(git_err(&args, &out)),
            }
        }
        Ok(())
    }

    pub fn fetch(&self) -> Result<()> {
        self.check_origin_url()?;
        self.run(&["fetch", "--quiet", "--prune", "origin"])
            .map(|_| ())
    }

    pub fn push(&self, branch: &str) -> Result<()> {
        self.check_origin_url()?;
        let refspec = format!("HEAD:refs/heads/{branch}");
        self.run(&["push", "--quiet", "origin", &refspec])?;
        // Track the remote branch so status reports ahead/behind.
        let _ = self.output(&["branch", "--set-upstream-to", &format!("origin/{branch}")]);
        Ok(())
    }
}
