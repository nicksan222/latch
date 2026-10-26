//! CLI ↔ agent transport.
//!
//! - Agent mode: `AgentClient::connect_or_spawn` (client kind `cli`).
//! - Standalone mode (`LATCH_PASSPHRASE_FILE` or `LATCH_IDENTITY_FILE` set):
//!   the agent's dispatcher runs in-process with `ClientCtx::cli()`, so both
//!   modes execute the exact same code path. The vault is unlocked on demand
//!   from the file.
//!
//! In both modes a `Locked` error on an interactive terminal triggers a
//! passphrase prompt, an unlock and one retry.

use std::io::{IsTerminal, Read};
use std::sync::Arc;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use zeroize::Zeroizing;

use super::CliError;
use latch::rpc::{
    Agent, AgentClient, AgentOptions, ClientCtx, ClientKind, ErrorCode, HelloParams, LocalClient,
    RpcError, UnlockParams, methods,
};

pub const ENV_PASSPHRASE_FILE: &str = "LATCH_PASSPHRASE_FILE";
pub const ENV_IDENTITY_FILE: &str = "LATCH_IDENTITY_FILE";

fn env_set(k: &str) -> Option<String> {
    std::env::var(k).ok().filter(|v| !v.is_empty())
}

pub fn standalone_requested() -> bool {
    env_set(ENV_PASSPHRASE_FILE).is_some() || env_set(ENV_IDENTITY_FILE).is_some()
}

enum Conn {
    Remote(AgentClient),
    Local {
        agent: Arc<Agent>,
        client: LocalClient,
    },
}

pub struct Session {
    conn: Conn,
}

/// Read the passphrase from `LATCH_PASSPHRASE_FILE`, stdin (`from_stdin`),
/// or an interactive prompt (twice when `confirm`). Never from argv.
pub fn read_passphrase(
    prompt: &str,
    confirm: bool,
    from_stdin: bool,
) -> Result<Zeroizing<String>, CliError> {
    if let Some(f) = env_set(ENV_PASSPHRASE_FILE) {
        let file = std::fs::File::open(&f)
            .map_err(|e| CliError::user(format!("cannot read {ENV_PASSPHRASE_FILE}: {e}")))?;
        return read_first_line(file)
            .map_err(|e| CliError::user(format!("cannot read {ENV_PASSPHRASE_FILE}: {e}")));
    }
    if from_stdin {
        return read_first_line(std::io::stdin().lock())
            .map_err(|e| CliError::user(format!("cannot read stdin: {e}")));
    }
    if !std::io::stdin().is_terminal() {
        return Err(CliError::user(
            "no passphrase source: run in a terminal, pass --passphrase-stdin, or set LATCH_PASSPHRASE_FILE",
        ));
    }
    let a = Zeroizing::new(
        rpassword::prompt_password(prompt).map_err(|e| CliError::user(format!("prompt: {e}")))?,
    );
    if confirm {
        let b = Zeroizing::new(
            rpassword::prompt_password("Repeat passphrase: ")
                .map_err(|e| CliError::user(format!("prompt: {e}")))?,
        );
        if *a != *b {
            return Err(CliError::user("passphrases do not match"));
        }
    }
    Ok(a)
}

/// Longest passphrase line accepted.
pub const MAX_PASSPHRASE_BYTES: usize = 4096;

/// Read the first line (without waiting for EOF) into zeroizing storage,
/// byte by byte so nothing past the newline is buffered. At most
/// [`MAX_PASSPHRASE_BYTES`].
pub fn read_first_line<R: Read>(mut r: R) -> std::io::Result<Zeroizing<String>> {
    let mut buf = Zeroizing::new(Vec::with_capacity(128));
    let mut b = [0u8; 1];
    loop {
        match r.read(&mut b) {
            Ok(0) => break,
            Ok(_) if b[0] == b'\n' => break,
            Ok(_) => {
                if buf.len() >= MAX_PASSPHRASE_BYTES {
                    zeroize::Zeroize::zeroize(&mut b);
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "passphrase line is too long",
                    ));
                }
                buf.push(b[0]);
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    zeroize::Zeroize::zeroize(&mut b);
    if buf.last() == Some(&b'\r') {
        buf.pop();
    }
    let s = std::str::from_utf8(&buf).map_err(|_| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, "passphrase is not UTF-8")
    })?;
    Ok(Zeroizing::new(s.to_owned()))
}

/// Unlock parameters from the standalone env files, if any.
fn standalone_unlock() -> Result<Option<UnlockParams>, CliError> {
    if let Some(f) = env_set(ENV_IDENTITY_FILE) {
        return Ok(Some(UnlockParams {
            passphrase: None,
            identity_file: Some(f),
            timeout_secs: None,
        }));
    }
    if env_set(ENV_PASSPHRASE_FILE).is_some() {
        let pw = read_passphrase("", false, false)?;
        return Ok(Some(UnlockParams {
            passphrase: Some(pw.to_string()),
            identity_file: None,
            timeout_secs: None,
        }));
    }
    Ok(None)
}

impl Session {
    /// Isolated in-process transport for action tests; never opens a socket.
    #[cfg(test)]
    pub(crate) fn for_test(agent: Arc<Agent>) -> Self {
        let client = LocalClient::new(&agent, ClientCtx::cli());
        Self {
            conn: Conn::Local { agent, client },
        }
    }

    /// Use an already connected agent without spawning or opening another connection.
    pub(crate) fn from_remote(client: AgentClient) -> Self {
        Self {
            conn: Conn::Remote(client),
        }
    }

    pub fn open() -> Result<Session, CliError> {
        let conn = if standalone_requested() {
            let agent = Agent::new(AgentOptions::default());
            let client = LocalClient::new(&agent, ClientCtx::cli());
            Conn::Local { agent, client }
        } else {
            let (c, _) = AgentClient::connect_or_spawn(HelloParams {
                client: ClientKind::Cli,
                version: env!("CARGO_PKG_VERSION").into(),
            })
            .map_err(CliError::from)?;
            Conn::Remote(c)
        };
        let mut s = Session { conn };
        if s.is_standalone() {
            // Standalone = one-shot unlocked session. Failures (no vault yet,
            // bad credentials) surface on the first real call instead.
            if let Ok(Some(p)) = standalone_unlock()
                && let Ok(v) = serde_json::to_value(&p)
            {
                let _ = s.raw(methods::VAULT_UNLOCK, v);
            }
        }
        Ok(s)
    }

    /// Private embedded agent for path-bound host commands that need no socket.
    pub fn open_local() -> Self {
        let agent = Agent::new(AgentOptions::default());
        let client = LocalClient::new(&agent, ClientCtx::cli());
        Self {
            conn: Conn::Local { agent, client },
        }
    }

    pub fn is_standalone(&self) -> bool {
        matches!(self.conn, Conn::Local { .. })
    }

    /// The in-process agent (standalone mode).
    pub fn local_agent(&self) -> Option<&Arc<Agent>> {
        match &self.conn {
            Conn::Local { agent, .. } => Some(agent),
            Conn::Remote(_) => None,
        }
    }

    pub fn raw(&mut self, method: &str, params: Value) -> Result<Value, RpcError> {
        match &mut self.conn {
            Conn::Remote(c) => c.call(method, params),
            Conn::Local { client, .. } => client.call(method, params),
        }
    }

    /// Call with automatic unlock (standalone file / interactive prompt)
    /// when the vault is locked.
    fn call<T: DeserializeOwned>(
        &mut self,
        method: &str,
        params: impl Serialize,
    ) -> Result<T, CliError> {
        let p = serde_json::to_value(params).map_err(|e| CliError::user(e.to_string()))?;
        let v = match self.raw(method, p.clone()) {
            Err(e) if e.code == ErrorCode::Locked && self.try_auto_unlock()? => {
                self.raw(method, p)?
            }
            r => r?,
        };
        serde_json::from_value(v)
            .map_err(|_| CliError::new(1, format!("unexpected response to {method}")))
    }

    fn try_auto_unlock(&mut self) -> Result<bool, CliError> {
        let params = match standalone_unlock()? {
            Some(p) => p,
            None if std::io::stdin().is_terminal() && std::io::stderr().is_terminal() => {
                let pw = read_passphrase("Vault passphrase: ", false, false)?;
                UnlockParams {
                    passphrase: Some(pw.to_string()),
                    identity_file: None,
                    timeout_secs: None,
                }
            }
            None => return Ok(false),
        };
        let p = serde_json::to_value(&params).map_err(|e| CliError::user(e.to_string()))?;
        self.raw(methods::VAULT_UNLOCK, p)?;
        Ok(true)
    }
}

// Keep SDK requests on the same transport and automatic-unlock path as CLI calls.
impl latch_rpc::sdk::RpcTransport for Session {
    type Error = CliError;

    fn request<P: Serialize, R: DeserializeOwned>(
        &mut self,
        method: &str,
        params: P,
    ) -> Result<R, Self::Error> {
        self.call::<R>(method, params)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use latch::agent::browser::ManagerConfig;
    use latch::agent::clipboard::MemoryClipboard;
    use latch::rpc::*;
    use std::time::Duration;

    #[test]
    fn generated_sdk_uses_real_session_dispatch_and_preserves_error_policy() {
        const TEST_NAME: &str = "cli::session::tests::generated_sdk_uses_real_session_dispatch_and_preserves_error_policy";
        const CHILD_TEST: &str = "LATCH_SDK_CHILD_TEST";
        const CHILD_ROOT: &str = "LATCH_SDK_CHILD_ROOT";
        if std::env::var(CHILD_TEST).as_deref() != Ok(TEST_NAME) {
            let root = tempfile::tempdir().unwrap();
            let git_config = root.path().join("gitconfig");
            let git_template = root.path().join("git-template");
            std::fs::write(&git_config, "").unwrap();
            std::fs::create_dir(&git_template).unwrap();
            let mut child = std::process::Command::new(std::env::current_exe().unwrap());
            // The dependency's trust store follows environment paths, not AgentOptions.
            // Isolate from birth; never mutate the multi-threaded test runner's env.
            for (name, _) in std::env::vars_os() {
                let key = name.to_string_lossy();
                if key.starts_with("GIT_") || key.starts_with("LATCH_") {
                    child.env_remove(name);
                }
            }
            let output = child
                .args(["--exact", TEST_NAME, "--test-threads=1", "--nocapture"])
                .current_dir(root.path())
                .env(CHILD_TEST, TEST_NAME)
                .env(CHILD_ROOT, root.path())
                .env("LATCH_CONFIG_DIR", root.path().join("config"))
                .env("LATCH_DATA_DIR", root.path().join("data"))
                .env("LATCH_RUNTIME_DIR", root.path().join("runtime"))
                .env("LATCH_RAM_DIR", root.path().join("ram"))
                .env("HOME", root.path())
                .env("XDG_CONFIG_HOME", root.path().join("xdg-config"))
                .env("GIT_CONFIG_GLOBAL", git_config)
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_TEMPLATE_DIR", git_template)
                .env("GIT_TERMINAL_PROMPT", "0")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "isolated SDK integration failed:\n{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(
                String::from_utf8_lossy(&output.stdout).contains("1 passed"),
                "child must execute the exact SDK integration test"
            );
            return;
        }
        let directory = std::path::PathBuf::from(std::env::var_os(CHILD_ROOT).unwrap());
        let agent = Agent::new(AgentOptions {
            config_dir: Some(directory.join("config")),
            clipboard: Some(Arc::new(MemoryClipboard::new())),
            browser_config: Some(ManagerConfig {
                ram_dir: directory.join("ram"),
                data_dir: directory.join("data"),
                browser_override: None,
                path_var: None,
                term_timeout: Duration::from_secs(1),
                archive_wait: Duration::from_secs(1),
            }),
            ..Default::default()
        });
        let mut session = Session::for_test(agent.clone());
        let result = (|| -> Result<(), CliError> {
            assert!(!session.vault_status()?.initialized);
            let initialized = session.vault_init(InitParams {
                path: directory.join("vault").display().to_string(),
                passphrase: "sdk integration test passphrase".into(),
                // Exercise production KDF validation without global test env changes.
                kdf: Some(KdfParams {
                    memory_kib: 64 * 1024,
                    iterations: 1,
                    parallelism: 1,
                }),
                git_name: Some("SDK integration test".into()),
                git_email: Some("sdk-test@example.invalid".into()),
                remote_url: None,
                recovery_kit: false,
            })?;
            assert!(initialized.status.initialized);
            assert!(!initialized.status.locked);
            assert!(!session.vault_trust()?.fingerprint.is_empty());
            let vault_id = initialized.status.vault_id.as_ref().unwrap();
            assert!(
                directory
                    .join("config/trust")
                    .join(format!("{vault_id}.json"))
                    .is_file(),
                "trust pins must stay under the isolated fixture config root"
            );
            // This adapter test owns CLI error presentation and session wiring.
            // CRUD/merge/clipboard policies are exercised by core RPC contracts.
            assert!(session.vault_lock()?.locked);
            let error = session.profiles_list().unwrap_err();
            assert_eq!(error.code, 2);
            assert!(error.message.ends_with(" (run `latch unlock`)"));
            Ok(())
        })();
        agent.shutdown();
        result.unwrap();
    }
}
