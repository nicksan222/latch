//! CLI in standalone mode (`LATCH_PASSPHRASE_FILE`): the dispatcher runs
//! in-process, no agent socket.

mod support;

use support::*;

const MASK: &str = "••••••";

fn setup_entries(e: &Env) {
    e.init();
    e.ok(&["profile", "add", "acme"]);
    e.ok(&[
        "set",
        "acme/GitHub",
        "username=alice",
        "password=gh-secret-pw",
        "url=https://github.com",
    ]);
    e.ok(&[
        "set",
        "acme/app",
        "DATABASE_URL=postgres://u:p@h/db",
        "PORT=3000",
    ]);
}

#[test]
fn full_flow() {
    let e = Env::standalone();
    setup_entries(&e);
    assert!(!e.path("r").join("agent.sock").exists(), "no agent spawned");

    let st = e.ok(&["status"]);
    assert!(st.contains("unlocked"), "{st}");
    let js = e.ok(&["status", "--json"]);
    let v: serde_json::Value = serde_json::from_str(&js).unwrap();
    assert_eq!(v["initialized"], true);

    let ls = e.ok(&["ls"]);
    assert!(
        ls.contains("acme/GitHub") && ls.contains("acme/app"),
        "{ls}"
    );
    let ls = e.ok(&["ls", "acme", "--type", "env", "--json"]);
    assert!(ls.contains("app") && !ls.contains("GitHub"));
    assert!(e.ok(&["profile", "ls"]).contains("acme"));
    assert!(e.ok(&["search", "git"]).contains("acme/GitHub"));

    // get: masked in non-TTY; --reveal needs --yes; --field --reveal prints.
    let out = e.ok(&["get", "acme/GitHub"]);
    assert!(out.contains(MASK) && out.contains("alice"), "{out}");
    assert!(!out.contains("gh-secret-pw"));
    let (code, out, err) = e.run(&["get", "acme/GitHub", "--reveal"]);
    assert_eq!(code, 1);
    assert!(!out.contains("gh-secret-pw") && err.contains("--yes"));
    assert!(
        e.ok(&["get", "acme/GitHub", "--reveal", "--yes"])
            .contains("gh-secret-pw")
    );
    // A single field is a secret too: same non-TTY guard.
    let (code, out, err) = e.run(&["get", "acme/github", "--field", "password", "--reveal"]);
    assert_eq!(code, 1);
    assert!(!out.contains("gh-secret-pw") && err.contains("--yes"));
    assert_eq!(
        e.ok(&[
            "get",
            "acme/github",
            "--field",
            "password",
            "--reveal",
            "--yes"
        ]),
        "gh-secret-pw\n"
    );
    let out = e.ok(&["get", "acme/GitHub", "--field", "password"]);
    assert!(!out.contains("gh-secret-pw"));
    assert_eq!(
        e.ok(&["get", "acme/GitHub", "--field", "username"]),
        "alice\n"
    );
    let j = e.ok(&["get", "acme/GitHub", "--json"]);
    assert!(!j.contains("gh-secret-pw") && j.contains("redactedFields"));
    let env_out = e.ok(&["get", "acme/app"]);
    assert!(!env_out.contains("postgres://"));

    // set: update keeps other secrets; --stdin value; types.
    e.ok(&["set", "acme/GitHub", "username=alice2"]);
    assert_eq!(
        e.ok(&[
            "get",
            "acme/GitHub",
            "--field",
            "password",
            "--reveal",
            "--yes"
        ]),
        "gh-secret-pw\n"
    );
    e.cmd()
        .args(["set", "acme/token", "--stdin"])
        .write_stdin("tok-from-stdin\n")
        .assert()
        .success();
    assert_eq!(
        e.ok(&["get", "acme/token", "--field", "value", "--reveal", "--yes"]),
        "tok-from-stdin\n"
    );
    std::fs::write(e.path("x.env"), "FROM_FILE=yes\n").unwrap();
    e.ok(&[
        "set",
        "acme/app",
        "--from-file",
        e.path("x.env").to_str().unwrap(),
    ]);
    e.ok(&[
        "set",
        "acme/db",
        "--type",
        "database",
        "host=db.internal",
        "port=5432",
        "username=app",
        "password=dbpw",
        "database=main",
    ]);
    assert!(
        e.ok(&["get", "acme/db", "--field", "url", "--reveal", "--yes"])
            .contains("db.internal")
    );
    assert_eq!(
        e.code(&["set", "acme/db", "--type", "login", "username=x"]),
        1
    );
    assert_eq!(e.code(&["set", "acme/weird", "lower=1", "Mixed=2"]), 1);

    // exec / totp / env
    let out = e.ok(&[
        "exec",
        "acme/token",
        "--",
        "sh",
        "-c",
        "printf %s \"$LATCH_SECRET\"",
    ]);
    assert_eq!(out, "tok-from-stdin");
    e.ok(&[
        "set",
        "acme/GitHub",
        "totp=GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ",
    ]);
    let code = e.ok(&["totp", "acme/GitHub"]);
    assert_eq!(code.trim().len(), 6);
    let out = e.ok(&[
        "env",
        "acme",
        "--",
        "sh",
        "-c",
        "echo X=$DATABASE_URL F=$FROM_FILE",
    ]);
    assert_eq!(out.trim(), "X=postgres://u:p@h/db F=yes");

    // mv, rm, history, restore, trash
    e.ok(&["profile", "add", "beta"]);
    e.ok(&["mv", "acme/token", "beta/api-token"]);
    assert_eq!(
        e.ok(&[
            "get",
            "beta/api-token",
            "--field",
            "value",
            "--reveal",
            "--yes"
        ]),
        "tok-from-stdin\n"
    );
    let h = e.ok(&["history", "acme/GitHub"]);
    assert!(h.lines().count() >= 3, "{h}");
    let first_commit = h
        .lines()
        .last()
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .to_owned();
    e.ok(&["rm", "beta/api-token"]);
    assert_eq!(e.code(&["get", "beta/api-token"]), 1);
    assert!(e.ok(&["trash"]).contains("api-token"));
    let hj = e.ok(&["history", "acme/GitHub", "--json"]);
    let items: Vec<serde_json::Value> = serde_json::from_str(&hj).unwrap();
    let full = items
        .iter()
        .map(|i| i["commit"].as_str().unwrap().to_owned())
        .find(|c| c.starts_with(&first_commit))
        .unwrap();
    e.ok(&["restore", "acme/GitHub", &full]);
    assert_eq!(
        e.ok(&["get", "acme/GitHub", "--field", "username"]),
        "alice\n"
    );

    // profiles
    e.ok(&["profile", "rename", "beta", "gamma"]);
    assert!(e.ok(&["profile", "show", "gamma"]).contains("gamma"));
    assert_eq!(
        e.code(&["profile", "rm", "gamma"]),
        1,
        "needs --yes without TTY"
    );
    e.ok(&["profile", "rm", "gamma", "--yes"]);
    assert_eq!(e.code(&["profile", "show", "gamma"]), 1);

    // recipients, doctor, misc
    assert!(e.ok(&["recipients", "ls"]).contains("passphrase"));
    e.ok(&["doctor"]);
    e.ok(&["migrate"]);
    assert!(e.ok(&["completion", "bash"]).contains("latch"));
    // Lock in standalone is harmless; next command unlocks from the file.
    e.ok(&["lock"]);
    e.ok(&["ls"]);
}

#[test]
fn env_isolate_and_print_formats() {
    let e = Env::standalone();
    setup_entries(&e);
    e.ok(&["set", "acme/extra", "EXTRA=1", "PORT=4000"]);

    // Later entries win, named entries in order.
    let out = e.ok(&[
        "env",
        "acme",
        "app",
        "extra",
        "--",
        "sh",
        "-c",
        "echo $PORT",
    ]);
    assert_eq!(out.trim(), "4000");
    let out = e.ok(&[
        "env",
        "acme",
        "extra",
        "app",
        "--",
        "sh",
        "-c",
        "echo $PORT",
    ]);
    assert_eq!(out.trim(), "3000");

    // Parent env inherited unless --isolate.
    let out = e
        .cmd()
        .env("PARENT_ONLY", "visible")
        .args([
            "env",
            "acme",
            "app",
            "--",
            "sh",
            "-c",
            "echo [$PARENT_ONLY] [$PORT] [${PATH:+path}]",
        ])
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "[visible] [3000] [path]"
    );
    let out = e
        .cmd()
        .env("PARENT_ONLY", "visible")
        .args([
            "env",
            "acme",
            "app",
            "--isolate",
            "--",
            "sh",
            "-c",
            "echo [$PARENT_ONLY] [$PORT] [${PATH:+path}]",
        ])
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "[] [3000] [path]"
    );

    // Exit code propagates.
    assert_eq!(e.code(&["env", "acme", "--", "sh", "-c", "exit 7"]), 7);

    let d = e.ok(&["env", "acme", "app", "--print"]);
    assert!(
        d.contains("DATABASE_URL=") && d.contains("PORT=3000"),
        "{d}"
    );
    assert!(!d.contains("export "));
    let x = e.ok(&["env", "acme", "app", "--print", "--format", "export"]);
    assert!(x.contains("export PORT="), "{x}");
    let j = e.ok(&["env", "acme", "app", "--print", "--format", "json"]);
    let v: serde_json::Value = serde_json::from_str(&j).unwrap();
    assert_eq!(v["DATABASE_URL"], "postgres://u:p@h/db");
    assert_eq!(e.code(&["env", "acme"]), 1, "needs a command or --print");
}

#[test]
fn exit_codes() {
    let e = Env::standalone();
    assert_eq!(e.code(&["ls"]), 1, "no vault");
    setup_entries(&e);
    assert_eq!(e.code(&["get", "acme/nope"]), 1);
    assert_eq!(e.code(&["get", "nope/x"]), 1);
    assert_eq!(e.code(&["get", "no-slash"]), 1);

    // Wrong passphrase file → user error.
    std::fs::write(e.path("bad"), "wrong passphrase\n").unwrap();
    let o = e
        .cmd()
        .env("LATCH_PASSPHRASE_FILE", e.path("bad"))
        .args(["ls"])
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(1));

    // Tampering → 3.
    let ids = e.ok(&["ls", "acme", "--json"]);
    let v: Vec<serde_json::Value> = serde_json::from_str(&ids).unwrap();
    let eid = v[0]["id"].as_str().unwrap();
    let pid = v[0]["profileId"].as_str().unwrap();
    let f = e.vault().join(format!("profiles/{pid}/entries/{eid}.age"));
    let mut b = std::fs::read(&f).unwrap();
    let n = b.len();
    b[n - 3] ^= 0x41;
    std::fs::write(&f, b).unwrap();
    assert_eq!(e.code(&["ls"]), 3);
    assert_eq!(e.code(&["status"]), 3);
    assert_eq!(e.code(&["doctor"]), 3);
    e.ok(&["doctor", "--fix"]);
    e.ok(&["ls"]);
}

#[test]
fn sync_conflict_exit_code_and_resolve() {
    let a = Env::standalone();
    setup_entries(&a);
    let remote = a.path("remote.git");
    assert!(
        git(
            a.dir.path(),
            &[
                "init",
                "--bare",
                "--quiet",
                "-b",
                "main",
                remote.to_str().unwrap()
            ]
        )
        .0
    );
    a.ok(&["sync", "--set-remote", remote.to_str().unwrap()]);
    a.ok(&["sync"]);

    let b = Env::standalone();
    let bv = b.vault();
    b.ok(&[
        "init",
        bv.to_str().unwrap(),
        "--clone",
        remote.to_str().unwrap(),
    ]);
    b.ok(&["set", "acme/GitHub", "password=from-b"]);
    b.ok(&["sync"]);

    a.ok(&["set", "acme/GitHub", "password=from-a"]);
    let (code, out, err) = a.run(&["sync"]);
    assert_eq!(code, 4, "{out}{err}");
    assert!(out.contains("acme/GitHub"), "{out}");
    a.ok(&["resolve", "acme/GitHub", "mine"]);
    a.ok(&["sync"]);
    assert_eq!(
        a.ok(&[
            "get",
            "acme/GitHub",
            "--field",
            "password",
            "--reveal",
            "--yes"
        ]),
        "from-a\n"
    );
    b.ok(&["sync", "--pull-only"]);
    assert_eq!(
        b.ok(&[
            "get",
            "acme/GitHub",
            "--field",
            "password",
            "--reveal",
            "--yes"
        ]),
        "from-a\n"
    );
}

#[test]
fn hook_rejects_plaintext_commit() {
    let e = Env::standalone();
    e.init();
    e.ok(&["hook", "install", "--path", e.vault().to_str().unwrap()]);
    let hook = std::fs::read_to_string(e.vault().join(".git/hooks/pre-commit")).unwrap();
    assert!(hook.contains("doctor --staged"));

    std::fs::write(
        e.vault().join("leak.txt"),
        "AWS_SECRET_ACCESS_KEY=wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY\n",
    )
    .unwrap();
    assert!(git(&e.vault(), &["add", "leak.txt"]).0);
    let (ok, out) = git(&e.vault(), &["commit", "-m", "oops"]);
    assert!(!ok, "commit must be rejected: {out}");
    // Direct staged scan agrees (run inside the vault).
    let o = e
        .cmd()
        .current_dir(e.vault())
        .args(["doctor", "--staged"])
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(1));
    assert!(git(&e.vault(), &["rm", "--cached", "-q", "leak.txt"]).0);
    std::fs::remove_file(e.vault().join("leak.txt")).unwrap();
    let o = e
        .cmd()
        .current_dir(e.vault())
        .args(["doctor", "--staged"])
        .output()
        .unwrap();
    assert!(o.status.success());

    e.ok(&["hook", "uninstall", "--path", e.vault().to_str().unwrap()]);
    assert!(!e.vault().join(".git/hooks/pre-commit").exists());
}
