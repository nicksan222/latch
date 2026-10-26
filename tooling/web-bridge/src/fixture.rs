use latch::{
    agent::{Agent, ClientCtx},
    protocol::RpcRequest,
};
use serde_json::{Value, json};
use std::{path::Path, sync::Arc};
pub const PASS: &str = "correct horse battery staple";
pub(crate) fn call(a: &Agent, method: &str, params: Value) -> Result<Value, ()> {
    let r = a.dispatch(
        &ClientCtx::desktop(),
        RpcRequest {
            id: 1,
            method: method.into(),
            params,
        },
    );
    if let Some(e) = r.error {
        eprintln!("fixture setup {method}: {:?}", e.code);
        Err(())
    } else {
        Ok(r.result.unwrap_or(Value::Null))
    }
}
pub fn browser(root: &Path) -> std::io::Result<()> {
    let p = root.join("fixture-browser");
    std::os::unix::fs::symlink(std::env::current_exe()?, p)
}
pub fn seed(a: &Arc<Agent>, root: &Path, fresh: bool, unlocked: bool) -> Result<Value, ()> {
    let mut metadata = json!({"passphrase":PASS,"profiles":{},"entries":{},"vaults":{"personal":root.join("vault-personal"),"employer":root.join("vault-employer")}});
    let key = latch::core::crypto::SecretKey32::generate();
    let identity = latch::core::crypto::identity_from_bytes(key.expose());
    metadata["recipientPublicKey"] = json!(latch::core::crypto::public_key(&identity));
    if fresh {
        return Ok(metadata);
    }
    for (vault, names) in [
        ("employer", vec!["employer"]),
        ("personal", vec!["acme", "side-project", "personal"]),
    ] {
        call(
            a,
            "vault.init",
            json!({"path":metadata["vaults"][vault],"passphrase":PASS,"kdf":{"memoryKib":65536,"iterations":1,"parallelism":1},"gitName":"Fixture","gitEmail":"fixture@example.invalid","remoteUrl":null,"recoveryKit":false}),
        )?;
        call(
            a,
            "vault.unlock",
            json!({"passphrase":PASS,"identityFile":null,"timeoutSecs":null}),
        )?;
        for name in names {
            let mut browser = serde_json::to_value(
                latch::core::domain::default_browser_settings(()).map_err(|_| ())?,
            )
            .map_err(|_| ())?;
            browser["engine"] = json!(if name == "personal" {
                "firefox"
            } else {
                "chromium"
            });
            browser["binary"] = json!(root.join("fixture-browser"));
            let profile = call(
                a,
                "profiles.create",
                json!({"name":name,"color":"#2563eb","icon":"briefcase","description":"Fictional development fixture","pinned":name=="acme","browser":browser}),
            )?;
            let id = profile["id"].clone();
            metadata["profiles"][name] = id.clone();
            for (entry, payload) in entries(name) {
                let input = json!({"profileId":id,"name":entry,"tags":[],"notes":"","favorite":entry=="GitHub"||entry=="Google","payload":payload});
                let created = call(a, "entries.create", input.clone())?;
                metadata["entries"][format!("{name}/{entry}")] = created["id"].clone();
                if entry == "GitHub" {
                    let mut update = input;
                    update["id"] = created["id"].clone();
                    update["payload"]["data"]["password"] = json!("hunter2-acme-Gh!");
                    call(
                        a,
                        "entries.update",
                        json!({"id":created["id"],"input":update}),
                    )?;
                }
            }
        }
        call(a, "vault.lock", json!({}))?;
    }
    if unlocked {
        call(
            a,
            "vault.unlock",
            json!({"passphrase":PASS,"identityFile":null,"timeoutSecs":null}),
        )?;
    }
    Ok(metadata)
}
fn login(username: &str, password: &str, url: &str) -> Value {
    json!({"type":"login","data":{"username":username,"password":password,"urls":[url],"totp":null,"custom":[]}})
}
fn entries(profile: &str) -> Vec<(&'static str, Value)> {
    match profile {
        "acme" => vec![
            (
                "GitHub",
                json!({"type":"login","data":{"username":"nick@acme.dev","password":"old-password-2025","urls":["https://github.com/acme"],"totp":{"secret":"JBSWY3DPEHPK3PXP","algorithm":"SHA1","digits":6,"period":30,"issuer":"GitHub"},"custom":[{"key":"Recovery","value":"abcd-efgh-ijkl","hidden":true},{"key":"Org","value":"acme","hidden":false}]}}),
            ),
            (
                "AWS prod",
                json!({"type":"env","data":{"vars":[{"key":"AWS_ACCESS_KEY_ID","value":"AKIAIOSFODNN7EXAMPLE","hidden":false,"comment":"deploy user"},{"key":"AWS_SECRET_ACCESS_KEY","value":"wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY","hidden":true,"comment":null},{"key":"AWS_REGION","value":"eu-west-1","hidden":false,"comment":null},{"key":"DATABASE_URL","value":"postgresql://app:s3cret@db.acme.internal:5432/app","hidden":true,"comment":null}]}}),
            ),
            (
                "Stripe",
                json!({"type":"api_key","data":{"key":"sk_live_51Hacme0000000000000000","secret":"whsec_acme_000000000000","endpoint":"https://api.stripe.com","expiresAt":null}}),
            ),
            (
                "staging.pem",
                json!({"type":"file","data":{"filename":"staging.pem","mime":"application/x-pem-file","contentBase64":"RmljdGlvbmFsIGNlcnRpZmljYXRlCg==","size":22}}),
            ),
            (
                "Runbook",
                json!({"type":"note","data":{"body":"## On-call\n\n1. Check Grafana\n2. Page the DBA if p99 > 2s"}}),
            ),
            (
                "Deploy key",
                json!({"type":"ssh_key","data":{"privateKey":"-----BEGIN OPENSSH PRIVATE KEY-----\nfictional-example-only\n-----END OPENSSH PRIVATE KEY-----","publicKey":"ssh-ed25519 fictional-example-only deploy@acme","passphrase":null,"comment":"deploy@acme"}}),
            ),
            (
                "Postgres prod",
                json!({"type":"database","data":{"engine":"postgres","host":"db.acme.internal","port":5432,"database":"app","username":"app","password":"s3cret-pg-pass","options":"sslmode=require"}}),
            ),
            (
                "Sentry DSN",
                json!({"type":"secret","data":{"value":"https://abc123@o0.ingest.sentry.io/42"}}),
            ),
        ],
        "personal" => vec![
            (
                "Google",
                login(
                    "nick@gmail.com",
                    "correct-horse-google",
                    "https://accounts.google.com",
                ),
            ),
            ("Bank PIN", json!({"type":"secret","data":{"value":"4921"}})),
            (
                "Wi-Fi",
                json!({"type":"note","data":{"body":"SSID: home\nPassword: hunter2hunter2"}}),
            ),
        ],
        "side-project" => vec![
            (
                "Vercel",
                login("nick", "vercel-pass-123!", "https://vercel.com"),
            ),
            (
                ".env.local",
                json!({"type":"env","data":{"vars":[{"key":"NEXT_PUBLIC_APP_URL","value":"http://localhost:3000","hidden":false,"comment":null},{"key":"OPENAI_API_KEY","value":"sk-proj-sideproject000","hidden":true,"comment":"dev key"}]}}),
            ),
            (
                "OpenAI",
                json!({"type":"api_key","data":{"key":"sk-proj-sideproject000","secret":null,"endpoint":"https://api.openai.com/v1","expiresAt":"2027-01-01T00:00:00Z"}}),
            ),
            (
                "Redis",
                json!({"type":"database","data":{"engine":"redis","host":"localhost","port":6379,"database":"0","username":"","password":"redis-pass","options":""}}),
            ),
        ],
        _ => vec![(
            "Okta",
            login("nick@corp.example", "okta-pass", "https://corp.okta.com"),
        )],
    }
}

/// Fixed, session-owned local repositories; no caller-provided Git input.
pub fn sync_scenario(
    a: &Agent,
    root: &Path,
    metadata: &Value,
    conflict: bool,
) -> Result<Value, ()> {
    let remote = root.join("sync-remote.git");
    let peer_path = root.join("sync-peer");
    let output = std::process::Command::new("git")
        .args(["init", "--bare", "--quiet", "-b", "main"])
        .arg(&remote)
        .output()
        .map_err(|_| ())?;
    if !output.status.success() {
        return Err(());
    }
    call(a, "sync.setRemote", json!({"url":remote}))?;
    call(a, "sync.run", json!({"mode":"both"}))?;
    let peer = Agent::new(latch::agent::AgentOptions {
        config_dir: Some(root.join("sync-peer-config")),
        clipboard: Some(Arc::new(latch::agent::clipboard::MemoryClipboard::new())),
        ..Default::default()
    });
    // Always shut down the peer even if fixture construction fails.
    let result = (|| {
        call(&peer, "vault.clone", json!({"url":remote,"path":peer_path}))?;
        call(
            &peer,
            "vault.unlock",
            json!({"passphrase":PASS,"identityFile":null,"timeoutSecs":null}),
        )?;
        let id = &metadata["entries"]["acme/GitHub"];
        let profile = &metadata["profiles"]["acme"];
        let input = call(&peer, "entries.get", json!({"id":id,"reveal":true}))?;
        let mut input = input;
        input["payload"]["data"]["password"] = json!("fixture-remote-password");
        call(&peer, "entries.update", json!({"id":id,"input":input}))?;
        call(&peer, "sync.run", json!({"mode":"both"}))?;
        if conflict {
            input["payload"]["data"]["password"] = json!("fixture-mine-password");
            call(a, "entries.update", json!({"id":id,"input":input}))?;
        }
        Ok(json!({"entryId":id,"profileId":profile}))
    })();
    peer.shutdown();
    result
}
