//! CLI regression tests for review findings: remote URL credentials never
//! echoed, `--passphrase-stdin` does not wait for EOF, duration overflow,
//! EPIPE on stdout, trust display/reset, insecure runtime dir and agent
//! binary lookup failing closed.
#![cfg(unix)]

mod support;

use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use support::*;

/// `std::process::Command` with the same isolated environment as
/// `Env::cmd` (for tests that need raw pipes).
fn std_cmd(e: &Env) -> Command {
    let mut c = Command::new(assert_cmd::cargo::cargo_bin("latch"));
    c.current_dir(e.dir.path())
        .env("LATCH_CONFIG_DIR", e.path("c"))
        .env("LATCH_DATA_DIR", e.path("d"))
        .env("LATCH_RUNTIME_DIR", e.path("r"))
        .env("LATCH_RAM_DIR", e.path("m"))
        .env("LATCH_ALLOW_WEAK_KDF", "1")
        .env("LATCH_AGENT_IDLE_EXIT_SECS", "60")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("HOME", e.dir.path())
        .env_remove("LATCH_PASSPHRASE_FILE")
        .env_remove("LATCH_IDENTITY_FILE")
        .env_remove("LATCH_AGENT_BIN");
    if e.standalone {
        c.env("LATCH_PASSPHRASE_FILE", e.path("pass"));
    }
    c
}

fn wait_exit(child: &mut std::process::Child, limit: Duration) -> std::process::ExitStatus {
    let start = Instant::now();
    loop {
        if let Some(st) = child.try_wait().unwrap() {
            return st;
        }
        if start.elapsed() > limit {
            let _ = child.kill();
            panic!("process did not exit within {limit:?}");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn remote_credentials_rejected_and_never_echoed() {
    let e = Env::standalone();
    e.init();
    let (code, out, err) = e.run(&[
        "sync",
        "--set-remote",
        "https://user:sentinel-token@example.test/repo.git",
    ]);
    assert_eq!(code, 1);
    assert!(
        !out.contains("sentinel-token") && !err.contains("sentinel-token"),
        "{err}"
    );
    let (code, out, err) = e.run(&[
        "sync",
        "--set-remote",
        "https://sentinel-tok@example.test/r.git",
    ]);
    assert_eq!(code, 1, "{err}");
    assert!(
        !out.contains("sentinel-tok") && !err.contains("sentinel-tok"),
        "{err}"
    );
    assert!(err.contains("credential helper"), "{err}");
    let config = std::fs::read_to_string(e.vault().join(".git/config")).unwrap();
    assert!(!config.contains("sentinel-token") && !config.contains("sentinel-tok"));
}

#[test]
fn passphrase_stdin_reads_one_line_without_eof() {
    let e = Env::agent();
    let mut child = std_cmd(&e)
        .args(["init", e.vault().to_str().unwrap(), "--passphrase-stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    writeln!(stdin, "{PASS}").unwrap();
    // stdin stays open: the CLI must not wait for EOF.
    let st = wait_exit(&mut child, Duration::from_secs(60));
    drop(stdin);
    assert!(st.success());
    assert!(e.vault().join(".latch").is_dir());
}

#[test]
fn durations_overflow_and_zero_are_rejected() {
    let e = Env::standalone();
    e.init();
    for t in ["18446744073709551615d", "0", "8d"] {
        let (code, _, err) = e.run(&["unlock", "--timeout", t]);
        assert_eq!(code, 1, "{t}: {err}");
        assert!(!err.contains("panicked"), "{err}");
    }
    let (code, _, err) = e.run(&["copy", "acme/x", "--clear", "99999999999999999999d"]);
    assert_eq!(code, 1);
    assert!(!err.contains("panicked"), "{err}");
}

#[test]
fn closed_stdout_pipe_exits_quietly() {
    let e = Env::standalone();
    e.init();
    for i in 0..30 {
        e.ok(&["profile", "add", &format!("p{i}")]);
    }
    let mut child = std_cmd(&e)
        .args(["profile", "ls"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // Like `latch profile ls | head -0`: the reader is gone before output.
    drop(child.stdout.take());
    let st = wait_exit(&mut child, Duration::from_secs(60));
    let mut err = String::new();
    std::io::Read::read_to_string(&mut child.stderr.take().unwrap(), &mut err).unwrap();
    assert!(
        !err.contains("panicked") && !err.contains("Broken pipe"),
        "{err}"
    );
    assert_eq!(st.code(), Some(0), "{err}");
}

#[test]
fn status_shows_fingerprint_and_trust_reset_reports_first_use() {
    let e = Env::standalone();
    e.init();
    let out = e.ok(&["status"]);
    let fp_line = out
        .lines()
        .find(|l| l.starts_with("fingerprint:"))
        .unwrap_or_else(|| panic!("{out}"))
        .to_owned();
    let show = e.ok(&["trust", "show"]);
    assert!(show.contains("fingerprint:"));
    // Unlocked (standalone auto-unlock): refused without --yes.
    let (code, _, err) = e.run(&["trust", "reset"]);
    assert_eq!(code, 4, "{err}");
    let (code, _, err) = e.run(&["trust", "reset", "--yes"]);
    assert_eq!(code, 0, "{err}");
    let (code, _, err) = e.run(&["unlock"]);
    assert_eq!(code, 0, "{err}");
    assert!(err.contains("vault fingerprint:"), "{err}");
    assert!(err.contains("compare it on another device"), "{err}");
    let fp = fp_line.split_whitespace().nth(1).unwrap();
    assert!(err.contains(fp), "{err}");
}

#[test]
fn insecure_runtime_dir_is_refused() {
    let e = Env::agent();
    // A runtime dir that is a symlink (e.g. planted by another user).
    let real = e.path("real-run");
    std::fs::create_dir_all(&real).unwrap();
    std::os::unix::fs::symlink(&real, e.path("r")).unwrap();
    let (code, _, err) = e.run(&["status"]);
    assert_ne!(code, 0);
    assert!(
        err.contains("insecure") || err.contains("refusing"),
        "{err}"
    );
    assert!(
        !real.join("agent.sock").exists(),
        "no agent was started there"
    );
}

#[test]
fn group_readable_runtime_dir_is_refused() {
    use std::os::unix::fs::PermissionsExt;
    let e = Env::agent();
    std::fs::create_dir_all(e.path("r")).unwrap();
    std::fs::set_permissions(e.path("r"), std::fs::Permissions::from_mode(0o755)).unwrap();
    let (code, _, err) = e.run(&["status"]);
    assert_ne!(code, 0);
    assert!(err.contains("insecure"), "{err}");
}

#[test]
fn agent_binary_never_from_path() {
    let e = Env::agent();
    // A relative LATCH_AGENT_BIN (would be resolved through PATH/cwd).
    let o = e
        .cmd()
        .env("LATCH_AGENT_BIN", "latch")
        .args(["status"])
        .output()
        .unwrap();
    assert!(!o.status.success());
    assert!(String::from_utf8_lossy(&o.stderr).contains("absolute path"));
    assert!(!e.path("r").join("agent.sock").exists());
}
