// Play RPC never resolves secrets into a webview response or launches while locked.
#[path = "../../../../tests/agent_support/mod.rs"]
mod agent_support;
use agent_support::*;
use serde_json::json;

#[test]
fn local_action_directory_is_per_vault_and_rejects_vault_paths() {
    let t = TestAgent::new();
    t.init();
    let p = t
        .call(
            "profiles.create",
            serde_json::to_value(profile_input("project")).unwrap(),
        )
        .unwrap();
    let id = p["id"].as_str().unwrap();
    let r = t.call("actions.list", json!({"id":id})).unwrap();
    assert_eq!(r, json!({"actions":[],"workingDirectory":null}));
    let e = t
        .call(
            "actions.configure",
            json!({"profileId":id,"workingDirectory":t.vault_path()}),
        )
        .unwrap_err();
    assert_eq!(e.code, latch::protocol::ErrorCode::InvalidParams);
    let folder = t.dir.path().join("project with spaces");
    std::fs::create_dir(&folder).unwrap();
    t.call(
        "actions.configure",
        json!({"profileId":id,"workingDirectory":folder}),
    )
    .unwrap();
    let r = t.call("actions.list", json!({"id":id})).unwrap();
    assert_eq!(r["workingDirectory"], folder.to_str().unwrap());
    t.call("vault.lock", json!({})).unwrap();
    let e = t
        .call(
            "actions.run",
            json!({"profileId":id,"actionId":"aaaaaaaaaaaaaaaaaaaaaaaaaa"}),
        )
        .unwrap_err();
    assert_eq!(e.code, latch::protocol::ErrorCode::Locked);
    t.agent.shutdown();
}

#[test]
fn command_run_requires_local_folder_and_browser_only_does_not() {
    let t = TestAgent::new();
    t.init();
    let p = t
        .call(
            "profiles.create",
            serde_json::to_value(profile_input("project")).unwrap(),
        )
        .unwrap();
    let id = p["id"].as_str().unwrap();
    let a=t.call("actions.save",json!({"profileId":id,"id":null,"input":{"name":"Dev","command":"npm run dev","envEntryIds":[],"envKeys":null,"urls":[]}})).unwrap();
    let e = t
        .call("actions.run", json!({"profileId":id,"actionId":a["id"]}))
        .unwrap_err();
    assert!(e.message.contains("project directory"));
    let b=t.call("actions.save",json!({"profileId":id,"id":null,"input":{"name":"Console","command":"","envEntryIds":[],"envKeys":null,"urls":["https://example.com"]}})).unwrap();
    // No browser is installed in this isolated fixture: failure must concern the browser, not a folder.
    let result = t.call("actions.run", json!({"profileId":id,"actionId":b["id"]}));
    if let Err(e) = result {
        assert!(!e.message.contains("project directory"));
    }
    t.agent.shutdown();
}

#[cfg(unix)]
#[test]
fn action_urls_use_managed_provider_and_require_closing_an_existing_session() {
    use latch::agent::browser::ManagerConfig;
    use std::time::{Duration, Instant};
    setup();
    let dir = tmpdir();
    let ready = dir.path().join("started");
    let args = dir.path().join("args");
    let fail = dir.path().join("fail");
    let bin = dir.path().join("fake-chromium");
    std::fs::write(&bin,format!("#!/bin/sh\nif [ -e '{}' ]; then exit 7; fi\nprintf '%s\\n' \"$@\" > '{}'\nif [ -e '{}' ]; then exit 0; fi\ntouch '{}'\ntrap 'exit 0' TERM\nwhile :; do sleep 0.05; done\n",fail.display(),args.display(),ready.display(),ready.display())).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
    let t = TestAgent::with(|o| {
        o.browser_config = Some(ManagerConfig {
            ram_dir: dir.path().join("ram"),
            data_dir: dir.path().join("data"),
            browser_override: Some(bin.clone()),
            path_var: None,
            term_timeout: Duration::from_secs(2),
            archive_wait: Duration::from_secs(2),
        })
    });
    t.init();
    let mut profile = profile_input("project");
    profile.browser.binary = Some(bin.to_string_lossy().into());
    let p = t
        .call("profiles.create", serde_json::to_value(&profile).unwrap())
        .unwrap();
    let id = p["id"].as_str().unwrap();
    let a=t.call("actions.save",json!({"profileId":id,"id":null,"input":{"name":"Consoles","command":"","envEntryIds":[],"envKeys":null,"urls":["https://example.com/one","http://localhost:3000/two"]}})).unwrap();
    let run = json!({"profileId":id,"actionId":a["id"]});
    t.call("actions.run", run.clone()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    while !ready.exists() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
    let logged = std::fs::read_to_string(&args).unwrap();
    assert!(
        logged.contains("https://example.com/one") && logged.contains("http://localhost:3000/two")
    );
    // A running session must retain the binary that actually owns its data directory.
    profile.browser.binary = Some("/usr/bin/false".into());
    t.call("profiles.update", json!({"id":id,"input":profile}))
        .unwrap();
    assert!(t.call("actions.run", run.clone()).is_err());
    t.agent.browsers().close(id).unwrap();
    // Legacy per-profile binary changes never select a host executable.
    t.call("actions.run", run).unwrap();
    t.agent.shutdown();
}
