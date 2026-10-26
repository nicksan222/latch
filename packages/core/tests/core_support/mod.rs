//! Shared helpers for `core_*` integration tests.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::Once;

use latch::core::{UnlockedVault, Vault, crypto};
use latch::protocol::*;

pub const PASS: &str = "correct horse battery staple";

static SETUP: Once = Once::new();

/// Must be called first in every test: allows weak KDF params and isolates
/// git from the developer's global/system config, and the Latch config dir
/// (trust pins, consent) and data dir (session archives) from the user's.
pub fn setup() {
    SETUP.call_once(|| {
        let base = std::env::temp_dir().join(format!(
            "latch-core-tests-{}-{}",
            std::process::id(),
            latch::core::ids::new_id()
        ));
        std::fs::create_dir_all(&base).unwrap();
        // SAFETY: runs once, before any test thread reads the environment
        // (every test calls `setup()` first and blocks on the Once).
        unsafe {
            if std::env::var_os("LATCH_CONFIG_DIR").is_none() {
                std::env::set_var("LATCH_CONFIG_DIR", base.join("config"));
            }
            if std::env::var_os("LATCH_DATA_DIR").is_none() {
                std::env::set_var("LATCH_DATA_DIR", base.join("data"));
            }
            std::env::set_var(crypto::ENV_ALLOW_WEAK_KDF, "1");
            std::env::set_var("GIT_CONFIG_GLOBAL", "/dev/null");
            std::env::set_var("GIT_CONFIG_NOSYSTEM", "1");
        }
    });
}

pub fn weak_kdf() -> KdfParams {
    crypto::WEAK_KDF
}

pub fn init_params(path: &Path) -> InitParams {
    InitParams {
        path: path.display().to_string(),
        passphrase: PASS.into(),
        kdf: Some(weak_kdf()),
        git_name: Some("Alice".into()),
        git_email: Some("alice@example.com".into()),
        remote_url: None,
        recovery_kit: false,
    }
}

pub fn new_vault(path: &Path) -> UnlockedVault {
    setup();
    Vault::init(&init_params(path)).expect("init").0
}

pub fn profile_input(name: &str) -> ProfileInput {
    ProfileInput {
        name: name.into(),
        color: "#2563eb".into(),
        icon: "briefcase".into(),
        description: format!("{name} work"),
        pinned: false,
        browser: BrowserSettings {
            engine: BrowserEngine::Chromium,
            binary: None,
            start_pages: vec!["https://example.com".into()],
            bookmarks: vec![Bookmark {
                title: "Jira".into(),
                url: "https://acme.atlassian.net".into(),
            }],
            extensions_allowed: true,
            autofill: AutofillScope::All,
            autofill_entry_ids: vec![],
        },
    }
}

pub fn entry_input(profile_id: &str, name: &str, payload: EntryPayload) -> EntryInput {
    EntryInput {
        profile_id: profile_id.into(),
        name: name.into(),
        tags: vec!["test".into()],
        notes: "some notes".into(),
        favorite: false,
        payload,
    }
}

pub const TOTP_SECRET: &str = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ"; // "12345678901234567890"

/// One payload of every entry type, all secrets populated.
pub fn all_payloads() -> Vec<(&'static str, EntryPayload)> {
    vec![
        (
            "GitHub",
            EntryPayload::Login(LoginData {
                username: "alice".into(),
                password: Some("gh-password-1".into()),
                urls: vec!["https://github.com".into()],
                totp: Some(TotpConfig {
                    secret: Some(TOTP_SECRET.into()),
                    algorithm: TotpAlgorithm::Sha1,
                    digits: 6,
                    period: 30,
                    issuer: Some("GitHub".into()),
                }),
                custom: vec![
                    CustomField { key: "Recovery".into(), value: Some("recovery-code-1".into()), hidden: true },
                    CustomField { key: "Org".into(), value: Some("acme".into()), hidden: false },
                ],
            }),
        ),
        ("Webhook secret", EntryPayload::Secret(SecretData { value: Some("whsec_123".into()) })),
        (
            "App env",
            EntryPayload::Env(EnvData {
                vars: vec![
                    EnvVar { key: "DATABASE_URL".into(), value: Some("postgres://u:p@h/db".into()), hidden: true, comment: Some("main db".into()) },
                    EnvVar { key: "PORT".into(), value: Some("3000".into()), hidden: false, comment: None },
                ],
            }),
        ),
        (
            "staging.pem",
            EntryPayload::File(FileData {
                filename: "staging.pem".into(),
                mime: "application/x-pem-file".into(),
                content_base64: Some("LS0tLS1CRUdJTiBLRVktLS0tLQo=".into()),
                size: 0,
            }),
        ),
        ("Runbook", EntryPayload::Note(NoteData { body: Some("# Steps\n1. do it".into()) })),
        (
            "Deploy key",
            EntryPayload::SshKey(SshKeyData {
                private_key: Some("-----BEGIN OPENSSH PRIVATE KEY-----\nabc\n-----END OPENSSH PRIVATE KEY-----\n".into()),
                public_key: "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIHsKLqeplhpW+uObz5dvMgjz1OxfM/XXUB+VHtZ6isGN deploy".into(),
                passphrase: Some("ssh-pass".into()),
                comment: "deploy@ci".into(),
            }),
        ),
        (
            "Prod DB",
            EntryPayload::Database(DatabaseData {
                engine: "postgres".into(),
                host: "db.internal".into(),
                port: Some(5432),
                database: "app".into(),
                username: "app".into(),
                password: Some("db-pass".into()),
                options: "sslmode=require".into(),
            }),
        ),
        (
            "Stripe",
            EntryPayload::ApiKey(ApiKeyData {
                key: Some("sk_test_key".into()),
                secret: Some("sk_test_secret".into()),
                endpoint: Some("https://api.stripe.com".into()),
                expires_at: Some("2027-01-01T00:00:00Z".into()),
            }),
        ),
    ]
}

/// Secret strings contained in `all_payloads` (must never leak).
pub const SECRET_MARKERS: &[&str] = &[
    "gh-password-1",
    "recovery-code-1",
    "whsec_123",
    "postgres://u:p@h/db",
    "LS0tLS1CRUdJTiBLRVktLS0tLQo=",
    "# Steps",
    "OPENSSH PRIVATE KEY",
    "ssh-pass",
    "db-pass",
    "sk_test_key",
    "sk_test_secret",
];

pub fn bare_remote() -> tempfile::TempDir {
    setup();
    let t = tempfile::tempdir().unwrap();
    let ok = std::process::Command::new("git")
        .args(["init", "--bare", "--quiet", "-b", "main"])
        .current_dir(t.path())
        .status()
        .unwrap()
        .success();
    assert!(ok);
    t
}

pub fn git(dir: &Path, args: &[&str]) -> String {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// Recursively copy a directory (skipping `.git`).
pub fn copy_dir(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for e in std::fs::read_dir(src).unwrap().flatten() {
        if e.file_name() == ".git" {
            continue;
        }
        let to = dst.join(e.file_name());
        if e.path().is_dir() {
            copy_dir(&e.path(), &to);
        } else {
            std::fs::copy(e.path(), to).unwrap();
        }
    }
}

pub fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/vault-v1")
}

pub fn unlock(path: &Path) -> UnlockedVault {
    setup();
    Vault::open(path).unwrap().unlock_passphrase(PASS).unwrap()
}
