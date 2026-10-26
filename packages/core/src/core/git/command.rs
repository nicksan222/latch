//! Shared non-interactive subprocess, environment and error policy.

use std::process::{Command, Output, Stdio};

use super::{Error, Git, Result};

pub(super) fn base_command() -> Command {
    let mut c = Command::new("git");
    c.env("GIT_TERMINAL_PROMPT", "0")
        .env("LC_ALL", "C")
        .env("GIT_EDITOR", "true")
        .env("GIT_MERGE_AUTOEDIT", "no")
        .args([
            "-c",
            "rerere.enabled=false",
            "-c",
            "core.quotepath=false",
            "-c",
            "color.ui=false",
            "-c",
            "commit.verbose=false",
            "-c",
            "advice.detachedHead=false",
            "-c",
            "core.autocrlf=false",
        ])
        .stdin(Stdio::null());
    c
}

pub(super) fn git_err(args: &[&str], out: &Output) -> Error {
    let stderr = String::from_utf8_lossy(&out.stderr);
    let first = stderr
        .lines()
        .filter(|l| !l.trim().is_empty())
        .take(3)
        .collect::<Vec<_>>()
        .join("; ");
    let cmd = args.first().copied().unwrap_or("");
    Error::Git(format!("git {cmd} failed: {}", first.trim()))
}

/// Is a `git` binary available?
pub fn available() -> bool {
    base_command()
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

impl Git {
    fn cmd(&self) -> Command {
        let mut c = base_command();
        c.current_dir(&self.root);
        c
    }

    pub fn output(&self, args: &[&str]) -> Result<Output> {
        self.cmd()
            .args(args)
            .output()
            .map_err(|e| Error::Git(format!("cannot run git: {e}")))
    }

    /// Run and return stdout bytes; non-zero exit → Error::Git.
    pub fn run_bytes(&self, args: &[&str]) -> Result<Vec<u8>> {
        let out = self.output(args)?;
        if !out.status.success() {
            return Err(git_err(args, &out));
        }
        Ok(out.stdout)
    }

    pub fn run(&self, args: &[&str]) -> Result<String> {
        Ok(String::from_utf8_lossy(&self.run_bytes(args)?).into_owned())
    }

    pub(super) fn ok(&self, args: &[&str]) -> bool {
        self.output(args).is_ok_and(|o| o.status.success())
    }

    /// Run with extra environment variables and optional stdin input.
    pub fn run_env(
        &self,
        args: &[&str],
        env: &[(&str, &std::ffi::OsStr)],
        input: Option<&[u8]>,
    ) -> Result<Vec<u8>> {
        use std::io::Write;
        let mut c = self.cmd();
        c.args(args)
            .envs(env.iter().map(|(k, v)| (*k, *v)))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if input.is_some() {
            c.stdin(Stdio::piped());
        }
        let mut child = c
            .spawn()
            .map_err(|e| Error::Git(format!("cannot run git: {e}")))?;
        let writer = match (input, child.stdin.take()) {
            (Some(bytes), Some(mut stdin)) => {
                let bytes = bytes.to_vec();
                Some(std::thread::spawn(move || {
                    let _ = stdin.write_all(&bytes);
                }))
            }
            _ => None,
        };
        let out = child
            .wait_with_output()
            .map_err(|e| Error::Git(format!("cannot run git: {e}")))?;
        if let Some(w) = writer {
            let _ = w.join();
        }
        if !out.status.success() {
            return Err(git_err(args, &out));
        }
        Ok(out.stdout)
    }
}
