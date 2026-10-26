use latch::agent::{Agent, ClientCtx};
use latch_rpc::{
    sdk::RpcSdk,
    v1::{self, entry_payload::Data},
};
use serde_json::{Value, json};
use std::{path::Path, sync::Arc};
pub const PASS: &str = "correct horse battery staple";
fn unlock() -> v1::UnlockParams {
    v1::UnlockParams {
        passphrase: Some(PASS.into()),
        ..Default::default()
    }
}
fn password(input: &mut v1::EntryInput, value: &str) {
    if let Some(v1::EntryPayload {
        data: Some(Data::Login(login)),
    }) = &mut input.payload
    {
        login.password = Some(value.into());
    }
}
pub fn browser(root: &Path) -> std::io::Result<()> {
    let p = root.join("fixture-browser");
    std::os::unix::fs::symlink(super::worker_executable()?, p)
}
pub fn seed(a: &Arc<Agent>, root: &Path, fresh: bool, unlocked: bool) -> Result<Value, ()> {
    let mut metadata = json!({"passphrase":PASS,"profiles":{},"entries":{},"vaults":{"personal":root.join("vault-personal"),"employer":root.join("vault-employer")}});
    let key = latch::core::crypto::SecretKey32::generate();
    let identity = latch::core::crypto::identity_from_bytes(key.expose());
    metadata["recipientPublicKey"] = json!(latch::core::crypto::public_key(&identity));
    if fresh {
        return Ok(metadata);
    }
    let mut rpc = latch::rpc::LocalClient::new(a, ClientCtx::desktop());
    for (vault, names) in [
        ("employer", vec!["employer"]),
        ("personal", vec!["acme", "side-project", "personal"]),
    ] {
        rpc.vault_init(v1::InitParams {
            path: metadata["vaults"][vault].as_str().ok_or(())?.into(),
            passphrase: PASS.into(),
            kdf: Some(v1::KdfParams {
                memory_kib: 65536,
                iterations: 1,
                parallelism: 1,
            }),
            git_name: Some("Fixture".into()),
            git_email: Some("fixture@example.invalid".into()),
            ..Default::default()
        })
        .map_err(|_| ())?;
        rpc.vault_unlock(unlock()).map_err(|_| ())?;
        for name in names {
            let defaults = rpc
                .domain_call(v1::DomainRequest {
                    operation: Some(v1::domain_request::Operation::DefaultBrowserSettings(
                        v1::Empty {},
                    )),
                })
                .map_err(|_| ())?;
            let Some(v1::domain_response::Operation::DefaultBrowserSettings(browser)) =
                defaults.operation
            else {
                return Err(());
            };
            let profile = rpc
                .profiles_create(v1::ProfileInput {
                    name: name.into(),
                    color: "#2563eb".into(),
                    icon: "briefcase".into(),
                    description: "Fictional development fixture".into(),
                    pinned: name == "acme",
                    browser: Some(browser),
                })
                .map_err(|_| ())?;
            metadata["profiles"][name] = json!(profile.id);
            for (entry, payload) in entries(name) {
                let mut input = v1::EntryInput {
                    profile_id: profile.id.clone(),
                    name: entry.into(),
                    favorite: entry == "GitHub" || entry == "Google",
                    payload: Some(payload),
                    ..Default::default()
                };
                let created = rpc.entries_create(input.clone()).map_err(|_| ())?;
                metadata["entries"][format!("{name}/{entry}")] = json!(created.id);
                if entry == "GitHub" {
                    password(&mut input, "hunter2-acme-Gh!");
                    rpc.entries_update(v1::EntryUpdateParams {
                        id: created.id,
                        input: Some(input),
                    })
                    .map_err(|_| ())?;
                }
            }
        }
        rpc.vault_lock().map_err(|_| ())?;
    }
    if unlocked {
        rpc.vault_unlock(unlock()).map_err(|_| ())?;
    }
    Ok(metadata)
}
fn payload(data: Data) -> v1::EntryPayload {
    v1::EntryPayload { data: Some(data) }
}
fn login(username: &str, password: &str, url: &str) -> v1::EntryPayload {
    payload(Data::Login(v1::LoginData {
        username: username.into(),
        password: Some(password.into()),
        urls: vec![url.into()],
        ..Default::default()
    }))
}
fn entries(profile: &str) -> Vec<(&'static str, v1::EntryPayload)> {
    match profile {
        "acme" => vec![
            (
                "GitHub",
                payload(Data::Login(v1::LoginData { username: "nick@acme.dev".into(), password: Some("old-password-2025".into()), urls: vec!["https://github.com/acme".into()], totp: Some(v1::TotpConfig { secret: Some("JBSWY3DPEHPK3PXP".into()), algorithm: v1::TotpAlgorithm::Sha1 as i32, digits: 6, period: 30, issuer: Some("GitHub".into()) }), custom: vec![v1::CustomField { key: "Recovery".into(), value: Some("abcd-efgh-ijkl".into()), hidden: true },v1::CustomField { key: "Org".into(), value: Some("acme".into()), hidden: false }] })),
            ),
            (
                "AWS prod",
                payload(Data::Env(v1::EnvData { vars: vec![v1::EnvVar { key: "AWS_ACCESS_KEY_ID".into(), value: Some("AKIAIOSFODNN7EXAMPLE".into()), hidden: false, comment: Some("deploy user".into()) },v1::EnvVar { key: "AWS_SECRET_ACCESS_KEY".into(), value: Some("wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".into()), hidden: true, comment: None },v1::EnvVar { key: "AWS_REGION".into(), value: Some("eu-west-1".into()), hidden: false, comment: None },v1::EnvVar { key: "DATABASE_URL".into(), value: Some("postgresql://app:s3cret@db.acme.internal:5432/app".into()), hidden: true, comment: None }] })),
            ),
            (
                "Stripe",
                payload(Data::ApiKey(v1::ApiKeyData { key: Some("sk_live_51Hacme0000000000000000".into()), secret: Some("whsec_acme_000000000000".into()), endpoint: Some("https://api.stripe.com".into()), expires_at: None })),
            ),
            (
                "staging.pem",
                payload(Data::File(v1::FileData { filename: "staging.pem".into(), mime: "application/x-pem-file".into(), content_base64: Some("RmljdGlvbmFsIGNlcnRpZmljYXRlCg==".into()), size: 22 })),
            ),
            (
                "Runbook",
                payload(Data::Note(v1::NoteData { body: Some("## On-call\n\n1. Check Grafana\n2. Page the DBA if p99 > 2s".into()) })),
            ),
            (
                "Deploy key",
                payload(Data::SshKey(v1::SshKeyData { private_key: Some("-----BEGIN OPENSSH PRIVATE KEY-----\nfictional-example-only\n-----END OPENSSH PRIVATE KEY-----".into()), public_key: "ssh-ed25519 fictional-example-only deploy@acme".into(), passphrase: None, comment: "deploy@acme".into() })),
            ),
            (
                "Postgres prod",
                payload(Data::Database(v1::DatabaseData { engine: "postgres".into(), host: "db.acme.internal".into(), port: Some(5432), database: "app".into(), username: "app".into(), password: Some("s3cret-pg-pass".into()), options: "sslmode=require".into() })),
            ),
            (
                "Sentry DSN",
                payload(Data::Secret(v1::SecretData { value: Some("https://abc123@o0.ingest.sentry.io/42".into()) })),
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
            ("Bank PIN", payload(Data::Secret(v1::SecretData { value: Some("4921".into()) }))),
            (
                "Wi-Fi",
                payload(Data::Note(v1::NoteData { body: Some("SSID: home\nPassword: hunter2hunter2".into()) })),
            ),
        ],
        "side-project" => vec![
            (
                "Vercel",
                login("nick", "vercel-pass-123!", "https://vercel.com"),
            ),
            (
                ".env.local",
                payload(Data::Env(v1::EnvData { vars: vec![v1::EnvVar { key: "NEXT_PUBLIC_APP_URL".into(), value: Some("http://localhost:3000".into()), hidden: false, comment: None },v1::EnvVar { key: "OPENAI_API_KEY".into(), value: Some("sk-proj-sideproject000".into()), hidden: true, comment: Some("dev key".into()) }] })),
            ),
            (
                "OpenAI",
                payload(Data::ApiKey(v1::ApiKeyData { key: Some("sk-proj-sideproject000".into()), secret: None, endpoint: Some("https://api.openai.com/v1".into()), expires_at: Some("2027-01-01T00:00:00Z".into()) })),
            ),
            (
                "Redis",
                payload(Data::Database(v1::DatabaseData { engine: "redis".into(), host: "localhost".into(), port: Some(6379), database: "0".into(), username: "".into(), password: Some("redis-pass".into()), options: "".into() })),
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
    a: &Arc<Agent>,
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
    let mut rpc = latch::rpc::LocalClient::new(a, ClientCtx::desktop());
    rpc.sync_set_remote(v1::SetRemoteParams {
        url: remote.to_string_lossy().into_owned(),
    })
    .map_err(|_| ())?;
    rpc.sync_run(v1::SyncParams {
        mode: v1::SyncMode::Both as i32,
    })
    .map_err(|_| ())?;
    let peer = Agent::new(latch::agent::AgentOptions {
        config_dir: Some(root.join("sync-peer-config")),
        clipboard: Some(Arc::new(latch::agent::clipboard::MemoryClipboard::new())),
        ..Default::default()
    });
    let result = (|| {
        let mut peer_rpc = latch::rpc::LocalClient::new(&peer, ClientCtx::desktop());
        peer_rpc
            .vault_clone(v1::CloneParams {
                url: remote.to_string_lossy().into_owned(),
                path: peer_path.to_string_lossy().into_owned(),
            })
            .map_err(|_| ())?;
        peer_rpc.vault_unlock(unlock()).map_err(|_| ())?;
        let id = metadata["entries"]["acme/GitHub"].as_str().ok_or(())?;
        let entry = peer_rpc
            .entries_get(v1::GetEntryParams {
                id: id.into(),
                reveal: true,
            })
            .map_err(|_| ())?;
        let mut input = v1::EntryInput {
            profile_id: entry.profile_id,
            name: entry.name,
            tags: entry.tags,
            notes: entry.notes,
            favorite: entry.favorite,
            payload: entry.payload,
        };
        password(&mut input, "fixture-remote-password");
        peer_rpc
            .entries_update(v1::EntryUpdateParams {
                id: id.into(),
                input: Some(input.clone()),
            })
            .map_err(|_| ())?;
        peer_rpc
            .sync_run(v1::SyncParams {
                mode: v1::SyncMode::Both as i32,
            })
            .map_err(|_| ())?;
        if conflict {
            password(&mut input, "fixture-mine-password");
            rpc.entries_update(v1::EntryUpdateParams {
                id: id.into(),
                input: Some(input),
            })
            .map_err(|_| ())?;
        }
        Ok(json!({"entryId":id,"profileId":metadata["profiles"]["acme"]}))
    })();
    peer.shutdown();
    result
}
