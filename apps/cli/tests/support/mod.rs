//! Helpers for `cli_*` tests: an isolated environment per test (config,
//! data, runtime dirs; weak KDF; git isolated). Agent-mode environments stop
//! (and if needed kill) their spawned agent on drop.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

pub const PASS: &str = "correct horse battery staple";

pub struct Env {
    pub dir: tempfile::TempDir,
    pub standalone: bool,
}

impl Env {
    pub fn standalone() -> Env {
        Self::new(true)
    }

    pub fn agent() -> Env {
        Self::new(false)
    }

    fn new(standalone: bool) -> Env {
        let dir = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap();
        std::fs::write(dir.path().join("pass"), format!("{PASS}\n")).unwrap();
        Env { dir, standalone }
    }

    pub fn path(&self, p: &str) -> PathBuf {
        self.dir.path().join(p)
    }

    pub fn vault(&self) -> PathBuf {
        self.path("vault")
    }

    pub fn cmd(&self) -> assert_cmd::Command {
        let mut c = assert_cmd::Command::cargo_bin("latch").unwrap();
        c.current_dir(self.dir.path())
            .env("LATCH_CONFIG_DIR", self.path("c"))
            .env("LATCH_DATA_DIR", self.path("d"))
            .env("LATCH_RUNTIME_DIR", self.path("r"))
            .env("LATCH_RAM_DIR", self.path("m"))
            .env("LATCH_ALLOW_WEAK_KDF", "1")
            .env("LATCH_AGENT_IDLE_EXIT_SECS", "60")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("HOME", self.dir.path())
            .env_remove("LATCH_PASSPHRASE_FILE")
            .env_remove("LATCH_IDENTITY_FILE")
            .env_remove("LATCH_AGENT_BIN");
        if self.standalone {
            c.env("LATCH_PASSPHRASE_FILE", self.path("pass"));
        }
        c
    }

    /// Run `latch <args>` and return (code, stdout, stderr).
    pub fn run(&self, args: &[&str]) -> (i32, String, String) {
        let o = self.cmd().args(args).output().unwrap();
        (
            o.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&o.stdout).into_owned(),
            String::from_utf8_lossy(&o.stderr).into_owned(),
        )
    }

    /// Run and assert success; returns stdout.
    pub fn ok(&self, args: &[&str]) -> String {
        let (code, out, err) = self.run(args);
        assert_eq!(code, 0, "latch {args:?} failed: {err}");
        out
    }

    pub fn code(&self, args: &[&str]) -> i32 {
        self.run(args).0
    }

    pub fn init(&self) {
        let v = self.vault();
        let mut c = self.cmd();
        c.args(["init", v.to_str().unwrap(), "--passphrase-stdin"])
            .write_stdin(format!("{PASS}\n"));
        let o = c.output().unwrap();
        assert!(
            o.status.success(),
            "init: {}",
            String::from_utf8_lossy(&o.stderr)
        );
    }

    fn pid_file(&self) -> PathBuf {
        self.path("r").join("agent.pid")
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        if self.standalone {
            return;
        }
        let _ = self.cmd().args(["agent", "stop"]).output();
        // Still there (stop failed): kill it.
        let pid = std::fs::read_to_string(self.pid_file())
            .ok()
            .and_then(|s| s.trim().parse::<i32>().ok());
        if let Some(pid) = pid {
            // SAFETY: kill(2) on the agent this test spawned.
            unsafe {
                libc::kill(pid, libc::SIGKILL);
            }
        }
    }
}

pub fn git(dir: &Path, args: &[&str]) -> (bool, String) {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@example.com")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@example.com")
        .output()
        .unwrap();
    (
        out.status.success(),
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
    )
}
