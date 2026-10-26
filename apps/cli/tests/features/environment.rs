//! CLI process behavior: --isolate, environment delivery and child exit forwarding.
//! Core owns environment resolution, merge and formatting acceptance.
#![cfg(unix)]
use super::cli_support::Env;

#[test]
fn standalone_process_injects_environment_into_child_without_ordinary_output_leaks() {
    let env = Env::standalone();
    env.init();
    env.ok(&["profile", "add", "acme"]);
    const SECRET: &str = "environment-feature-private-token";
    let (code, out, err) = env.run(&[
        "set",
        "acme/runtime",
        &format!("LATCH_FEATURE_TOKEN={SECRET}"),
    ]);
    assert_eq!(code, 0);
    assert!(!out.contains(SECRET) && !err.contains(SECRET));
    for args in [
        vec!["ls", "acme", "--json"],
        vec!["get", "acme/runtime", "--json"],
    ] {
        let (code, out, err) = env.run(&args);
        assert_eq!(code, 0);
        assert!(!out.contains(SECRET) && !err.contains(SECRET));
    }
    // The child validates the value without printing it. A parent sentinel
    // proves --isolate is applied, and the exit status proves child execution.
    let output = env.cmd().env("LATCH_FEATURE_PARENT", "must-not-reach-child")
        .args(["env", "acme", "runtime", "--isolate", "--", "sh", "-c",
            "test \"$LATCH_FEATURE_TOKEN\" = environment-feature-private-token && test -z \"${LATCH_FEATURE_PARENT:-}\" || exit 9; printf child-ran; exit 7"])
        .output().unwrap();
    assert_eq!(output.status.code(), Some(7));
    assert_eq!(output.stdout, b"child-ran");
    assert!(!String::from_utf8_lossy(&output.stderr).contains(SECRET));
}
