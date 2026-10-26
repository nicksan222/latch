//! CLI against a spawned headless agent (socket mode). Each test has its
//! own runtime dir; the agent is stopped/killed when the `Env` drops.
#![cfg(unix)]

mod support;

use support::*;

#[test]
fn agent_mode_flow() {
    let e = Env::agent();
    assert_eq!(e.code(&["agent", "status"]), 1, "not running yet");
    e.init();
    let sock = e.path("r").join("agent.sock");
    assert!(sock.exists(), "first command spawned the agent");
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&sock).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
    assert!(e.ok(&["agent", "status"]).contains("unlocked"));

    // State lives in the agent across CLI invocations.
    e.ok(&["profile", "add", "acme"]);
    e.ok(&["set", "acme/svc", "password=agent-secret", "username=svc"]);
    e.ok(&["set", "acme/app", "TOKEN=t0k3n"]);
    let out = e.ok(&["get", "acme/svc"]);
    assert!(!out.contains("agent-secret"));
    assert_eq!(e.code(&["get", "acme/svc", "--reveal"]), 1);
    assert!(
        e.ok(&["get", "acme/svc", "--reveal", "--yes"])
            .contains("agent-secret")
    );
    let out = e.ok(&["env", "acme", "--", "sh", "-c", "echo $TOKEN"]);
    assert_eq!(out.trim(), "t0k3n");

    // Locked → exit 2 (no TTY, so no prompt).
    e.ok(&["lock"]);
    let (code, _, err) = e.run(&["ls"]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("locked"));
    assert_eq!(e.code(&["env", "acme", "--", "true"]), 2);
    assert!(e.ok(&["status"]).contains("locked"));

    // Wrong passphrase → 1; right one unlocks.
    e.cmd()
        .args(["unlock", "--passphrase-stdin"])
        .write_stdin("not the passphrase\n")
        .assert()
        .code(1);
    // No passphrase source at all → 1.
    assert_eq!(e.code(&["unlock"]), 1);
    e.cmd()
        .args(["unlock", "--passphrase-stdin", "--timeout", "10m"])
        .write_stdin(format!("{PASS}\n"))
        .assert()
        .success();
    let st = e.ok(&["status", "--json"]);
    let v: serde_json::Value = serde_json::from_str(&st).unwrap();
    assert!(!v["locked"].as_bool().unwrap_or(false));
    let locks_in = v["locksInSecs"]
        .as_u64()
        .or_else(|| v["locksInSecs"].as_str().and_then(|s| s.parse().ok()))
        .unwrap();
    assert!(locks_in <= 600);
    assert_eq!(
        e.ok(&[
            "get", "acme/svc", "--field", "password", "--reveal", "--yes"
        ]),
        "agent-secret\n"
    );

    // Stop.
    e.ok(&["agent", "stop"]);
    assert!(!sock.exists());
    assert_eq!(e.code(&["agent", "status"]), 1);
    // Next command respawns a (locked) agent.
    assert_eq!(e.code(&["ls"]), 2);
}

#[test]
fn second_agent_refuses_to_run() {
    let e = Env::agent();
    e.ok(&["status"]); // spawns
    let (code, _, err) = e.run(&["agent", "run"]);
    assert_eq!(code, 1);
    assert!(err.contains("already running"), "{err}");
}
