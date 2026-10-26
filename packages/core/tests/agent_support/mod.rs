//! Shared helpers for `agent_*` integration tests: an in-process agent with
//! its own config dir and an in-memory clipboard.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Once};
use std::time::{Duration, Instant};

use latch::agent::clipboard::{ClipboardBackend, MemoryClipboard};
use latch::agent::{Agent, AgentOptions, ClientCtx};
use latch::core::crypto;
use latch::protocol::*;
use serde_json::{Value, json};

pub const PASS: &str = "correct horse battery staple";

static SETUP: Once = Once::new();

/// Weak KDF, git isolated from the developer's config, and private
/// data/runtime dirs for this test binary.
pub fn setup() {
    SETUP.call_once(|| {
        let base = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
            "la{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .subsec_micros()
        ));
        std::fs::create_dir_all(&base).unwrap();
        // SAFETY: runs once before any test thread reads these variables
        // (every test calls `setup()` first and blocks on the Once).
        unsafe {
            std::env::set_var(crypto::ENV_ALLOW_WEAK_KDF, "1");
            std::env::set_var("GIT_CONFIG_GLOBAL", "/dev/null");
            std::env::set_var("GIT_CONFIG_NOSYSTEM", "1");
            std::env::set_var("LATCH_DATA_DIR", base.join("data"));
            std::env::set_var("LATCH_RUNTIME_DIR", base.join("run"));
            std::env::set_var("LATCH_RAM_DIR", base.join("ram"));
            std::env::set_var("LATCH_CONFIG_DIR", base.join("config-unused"));
        }
    });
}

/// Temp dir under the cargo target dir (keeps /tmp clean).
pub fn tmpdir() -> tempfile::TempDir {
    tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap()
}

pub struct TestAgent {
    pub agent: Arc<Agent>,
    pub dir: tempfile::TempDir,
    pub clip: Arc<MemoryClipboard>,
    next_id: Mutex<u64>,
}

impl TestAgent {
    pub fn new() -> Self {
        Self::with(|_| {})
    }

    pub fn with(f: impl FnOnce(&mut AgentOptions)) -> Self {
        setup();
        let dir = tmpdir();
        let clip = Arc::new(MemoryClipboard::new());
        let mut opts = AgentOptions {
            config_dir: Some(dir.path().join("config")),
            clipboard: Some(clip.clone() as Arc<dyn ClipboardBackend>),
            tick: Duration::from_millis(50),
            ..Default::default()
        };
        f(&mut opts);
        if opts.browser_config.is_none() {
            opts.browser_config = Some(latch::agent::browser::ManagerConfig {
                ram_dir: dir.path().join("ram"),
                data_dir: dir.path().join("data"),
                browser_override: Some(dir.path().join("missing-fixture-browser")),
                path_var: None,
                term_timeout: Duration::from_millis(100),
                archive_wait: Duration::from_secs(2),
            });
        }
        Self {
            agent: Agent::new(opts),
            dir,
            clip,
            next_id: Mutex::new(1),
        }
    }

    pub fn vault_path(&self) -> PathBuf {
        self.dir.path().join("vault")
    }

    pub fn call_as(&self, ctx: &ClientCtx, method: &str, params: Value) -> Result<Value, RpcError> {
        let id = {
            let mut g = self.next_id.lock().unwrap();
            *g += 1;
            *g
        };
        let r = self.agent.dispatch(
            ctx,
            RpcRequest {
                id,
                method: method.into(),
                params,
            },
        );
        assert_eq!(r.id, id);
        match (r.result, r.error) {
            (_, Some(e)) => Err(e),
            (v, None) => Ok(v.unwrap_or(Value::Null)),
        }
    }

    pub fn call(&self, method: &str, params: Value) -> Result<Value, RpcError> {
        self.call_as(&ClientCtx::desktop(), method, params)
    }

    pub fn ok<T: serde::de::DeserializeOwned>(&self, method: &str, params: Value) -> T {
        let v = self
            .call(method, params)
            .unwrap_or_else(|e| panic!("{method} failed: {e:?}"));
        serde_json::from_value(v).unwrap_or_else(|e| panic!("{method} decode: {e}"))
    }

    pub fn err(&self, method: &str, params: Value) -> ErrorCode {
        self.call(method, params)
            .err()
            .unwrap_or_else(|| panic!("{method} unexpectedly succeeded"))
            .code
    }

    /// `vault.init` with weak KDF; returns the init result.
    pub fn init(&self) -> InitResult {
        self.ok(
            methods::VAULT_INIT,
            json!({
                "path": self.vault_path().display().to_string(),
                "passphrase": PASS,
                "kdf": crypto::WEAK_KDF,
                "gitName": "Alice",
                "gitEmail": "alice@example.com",
                "remoteUrl": null,
                "recoveryKit": false,
            }),
        )
    }

    pub fn unlock(&self) -> VaultStatus {
        self.ok(
            methods::VAULT_UNLOCK,
            json!({ "passphrase": PASS, "identityFile": null, "timeoutSecs": null }),
        )
    }

    pub fn create_profile(&self, name: &str) -> Profile {
        self.ok(
            methods::PROFILES_CREATE,
            serde_json::to_value(profile_input(name)).unwrap(),
        )
    }

    pub fn create_login(
        &self,
        profile_id: &str,
        name: &str,
        urls: &[&str],
        password: &str,
    ) -> Entry {
        self.ok(
            methods::ENTRIES_CREATE,
            serde_json::to_value(EntryInput {
                profile_id: profile_id.into(),
                name: name.into(),
                tags: vec![],
                notes: String::new(),
                favorite: true,
                payload: EntryPayload::Login(LoginData {
                    username: format!("{name}-user"),
                    password: Some(password.into()),
                    urls: urls.iter().map(|s| s.to_string()).collect(),
                    totp: Some(TotpConfig {
                        secret: Some(TOTP_SECRET.into()),
                        algorithm: TotpAlgorithm::Sha1,
                        digits: 6,
                        period: 30,
                        issuer: None,
                    }),
                    custom: vec![],
                }),
            })
            .unwrap(),
        )
    }
}

impl Drop for TestAgent {
    fn drop(&mut self) {
        self.agent.shutdown();
    }
}

pub const TOTP_SECRET: &str = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";

pub fn profile_input(name: &str) -> ProfileInput {
    ProfileInput {
        name: name.into(),
        color: "#2563eb".into(),
        icon: "briefcase".into(),
        description: String::new(),
        pinned: false,
        browser: BrowserSettings {
            engine: BrowserEngine::Chromium,
            binary: None,
            start_pages: vec![],
            bookmarks: vec![],
            extensions_allowed: false,
            autofill: AutofillScope::All,
            autofill_entry_ids: vec![],
        },
    }
}

/// Poll until `f` holds or `timeout` passes.
pub fn wait_until(timeout: Duration, mut f: impl FnMut() -> bool) -> bool {
    let end = Instant::now() + timeout;
    while Instant::now() < end {
        if f() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    f()
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
