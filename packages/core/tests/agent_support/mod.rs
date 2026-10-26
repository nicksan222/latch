//! Shared helpers for `agent_*` integration tests: an in-process agent with
//! its own config dir and an in-memory clipboard.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::{Arc, Once};
use std::time::{Duration, Instant};

use latch::agent::clipboard::{ClipboardBackend, MemoryClipboard};
use latch::agent::{Agent, AgentOptions, ClientCtx};
use latch::core::crypto;
use latch::protocol::*;
use latch_rpc::{
    sdk::{Call, Reply},
    v1,
};
use serde_json::{Value, json};

/// Legacy method names retained only for fixture-driven core tests. The test
/// adapter below turns them into generated typed calls before reaching Agent.
pub mod methods {
    pub const DOMAIN_CALL: &str = "domain.call";
    pub const HELLO: &str = "hello";
    pub const VAULT_STATUS: &str = "vault.status";
    pub const VAULT_INIT: &str = "vault.init";
    pub const VAULT_CLONE: &str = "vault.clone";
    pub const VAULT_OPEN: &str = "vault.open";
    pub const VAULT_LIST: &str = "vault.list";
    pub const VAULT_UNLOCK: &str = "vault.unlock";
    pub const VAULT_LOCK: &str = "vault.lock";
    pub const VAULT_BENCHMARK_KDF: &str = "vault.benchmarkKdf";
    pub const VAULT_CHANGE_PASSPHRASE: &str = "vault.changePassphrase";
    pub const VAULT_TRUST: &str = "vault.trust";
    pub const VAULT_FORGET_TRUST: &str = "vault.forgetTrust";
    pub const PROFILES_LIST: &str = "profiles.list";
    pub const PROFILES_GET: &str = "profiles.get";
    pub const PROFILES_CREATE: &str = "profiles.create";
    pub const PROFILES_UPDATE: &str = "profiles.update";
    pub const PROFILES_DELETE: &str = "profiles.delete";
    pub const ENTRIES_LIST: &str = "entries.list";
    pub const ENTRIES_GET: &str = "entries.get";
    pub const ENTRIES_REVEAL: &str = "entries.reveal";
    pub const ENTRIES_CREATE: &str = "entries.create";
    pub const ENTRIES_UPDATE: &str = "entries.update";
    pub const ENTRIES_DELETE: &str = "entries.delete";
    pub const ENTRIES_MOVE: &str = "entries.move";
    pub const ENTRIES_HISTORY: &str = "entries.history";
    pub const ENTRIES_RESTORE: &str = "entries.restore";
    pub const ENTRIES_TRASH: &str = "entries.trash";
    pub const ACTIONS_LIST: &str = "actions.list";
    pub const ACTIONS_SAVE: &str = "actions.save";
    pub const ACTIONS_DELETE: &str = "actions.delete";
    pub const ACTIONS_CONFIGURE: &str = "actions.configure";
    pub const ACTIONS_RUN: &str = "actions.run";
    pub const ENV_RESOLVE: &str = "env.resolve";
    pub const ENV_FORMAT: &str = "env.format";
    pub const ENV_PARSE: &str = "env.parse";
    pub const ENV_COPY: &str = "env.copy";
    pub const TOTP_GET: &str = "totp.get";
    pub const TOTP_PINNED: &str = "totp.pinned";
    pub const CLIPBOARD_COPY: &str = "clipboard.copy";
    pub const BROWSER_LAUNCH: &str = "browser.launch";
    pub const BROWSER_LIST: &str = "browser.list";
    pub const BROWSER_CLOSE: &str = "browser.close";
    pub const BROWSER_RESET: &str = "browser.reset";
    pub const BROWSER_DETECT: &str = "browser.detect";
    pub const BROWSER_SESSION_INFO: &str = "browser.sessionInfo";
    pub const SYNC_STATUS: &str = "sync.status";
    pub const SYNC_RUN: &str = "sync.run";
    pub const SYNC_CONFLICT: &str = "sync.conflict";
    pub const SYNC_RESOLVE: &str = "sync.resolve";
    pub const SYNC_SET_REMOTE: &str = "sync.setRemote";
    pub const RECIPIENTS_LIST: &str = "recipients.list";
    pub const RECIPIENTS_ADD: &str = "recipients.add";
    pub const RECIPIENTS_REMOVE: &str = "recipients.remove";
    pub const RECIPIENTS_ROTATE: &str = "recipients.rotate";
    pub const IMPORT_PREVIEW: &str = "import.preview";
    pub const IMPORT_APPLY: &str = "import.apply";
    pub const EXPORT_RUN: &str = "export.run";
    pub const DOCTOR_RUN: &str = "doctor.run";
    pub const SETTINGS_GET: &str = "settings.get";
    pub const SETTINGS_UPDATE: &str = "settings.update";
    pub const VAULT_LOCATE: &str = "vault.locate";
    pub const DOCTOR_SCAN: &str = "doctor.scan";
    pub const HOOKS_INSTALL: &str = "hooks.install";
    pub const HOOKS_UNINSTALL: &str = "hooks.uninstall";
    pub const TOTP_PARSE: &str = "totp.parse";
    pub const ALL: &[&str] = &[
        DOMAIN_CALL,
        HELLO,
        VAULT_STATUS,
        VAULT_INIT,
        VAULT_CLONE,
        VAULT_OPEN,
        VAULT_LIST,
        VAULT_UNLOCK,
        VAULT_LOCK,
        VAULT_BENCHMARK_KDF,
        VAULT_CHANGE_PASSPHRASE,
        VAULT_TRUST,
        VAULT_FORGET_TRUST,
        PROFILES_LIST,
        PROFILES_GET,
        PROFILES_CREATE,
        PROFILES_UPDATE,
        PROFILES_DELETE,
        ENTRIES_LIST,
        ENTRIES_GET,
        ENTRIES_REVEAL,
        ENTRIES_CREATE,
        ENTRIES_UPDATE,
        ENTRIES_DELETE,
        ENTRIES_MOVE,
        ENTRIES_HISTORY,
        ENTRIES_RESTORE,
        ENTRIES_TRASH,
        ACTIONS_LIST,
        ACTIONS_SAVE,
        ACTIONS_DELETE,
        ACTIONS_CONFIGURE,
        ACTIONS_RUN,
        ENV_RESOLVE,
        ENV_FORMAT,
        ENV_PARSE,
        ENV_COPY,
        TOTP_GET,
        TOTP_PINNED,
        CLIPBOARD_COPY,
        BROWSER_LAUNCH,
        BROWSER_LIST,
        BROWSER_CLOSE,
        BROWSER_RESET,
        BROWSER_DETECT,
        BROWSER_SESSION_INFO,
        SYNC_STATUS,
        SYNC_RUN,
        SYNC_CONFLICT,
        SYNC_RESOLVE,
        SYNC_SET_REMOTE,
        RECIPIENTS_LIST,
        RECIPIENTS_ADD,
        RECIPIENTS_REMOVE,
        RECIPIENTS_ROTATE,
        IMPORT_PREVIEW,
        IMPORT_APPLY,
        EXPORT_RUN,
        DOCTOR_RUN,
        SETTINGS_GET,
        SETTINGS_UPDATE,
        VAULT_LOCATE,
        DOCTOR_SCAN,
        HOOKS_INSTALL,
        HOOKS_UNINSTALL,
        TOTP_PARSE,
    ];
}

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
                term_timeout: Duration::from_millis(100),
                archive_wait: Duration::from_secs(2),
            });
        }
        Self {
            agent: Agent::new(opts),
            dir,
            clip,
        }
    }

    pub fn vault_path(&self) -> PathBuf {
        self.dir.path().join("vault")
    }

    pub fn call_as(&self, ctx: &ClientCtx, method: &str, params: Value) -> Result<Value, RpcError> {
        let call = test_call(method, params)?;
        let reply = self.agent.execute(ctx, call)?;
        test_reply(reply)
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

fn decode<T: serde::de::DeserializeOwned>(method: &str, value: Value) -> Result<T, RpcError> {
    let value = if value.is_null() { json!({}) } else { value };
    serde_json::from_value(value).map_err(|_| RpcError {
        code: ErrorCode::InvalidParams,
        message: format!("invalid parameters: {method}: malformed parameters"),
    })
}

fn empty(method: &str, params: Value) -> Result<v1::Empty, RpcError> {
    if params.is_null() || params.as_object().is_some_and(serde_json::Map::is_empty) {
        Ok(v1::Empty {})
    } else {
        Err(RpcError {
            code: ErrorCode::InvalidParams,
            message: format!("invalid parameters: {method}: malformed parameters"),
        })
    }
}

pub fn test_call(method: &str, params: Value) -> Result<Call, RpcError> {
    use methods as m;
    macro_rules! call {
        ($model:ty, $variant:ident) => {
            Call::$variant(decode::<$model>(method, params)?.into())
        };
    }
    let call = match method {
        m::DOMAIN_CALL => call!(DomainRequest, DomainCall),
        m::HELLO => call!(HelloParams, Hello),
        m::VAULT_STATUS => Call::VaultStatus(empty(method, params)?),
        m::VAULT_INIT => call!(InitParams, VaultInit),
        m::VAULT_CLONE => call!(CloneParams, VaultClone),
        m::VAULT_OPEN => call!(OpenParams, VaultOpen),
        m::VAULT_LIST => Call::VaultList(empty(method, params)?),
        m::VAULT_UNLOCK => call!(UnlockParams, VaultUnlock),
        m::VAULT_LOCK => Call::VaultLock(empty(method, params)?),
        m::VAULT_BENCHMARK_KDF => call!(KdfBenchmarkParams, VaultBenchmarkKdf),
        m::VAULT_CHANGE_PASSPHRASE => call!(ChangePassphraseParams, VaultChangePassphrase),
        m::VAULT_TRUST => Call::VaultTrust(empty(method, params)?),
        m::VAULT_FORGET_TRUST => call!(ForgetTrustParams, VaultForgetTrust),
        m::PROFILES_LIST => Call::ProfilesList(empty(method, params)?),
        m::PROFILES_GET => call!(IdParams, ProfilesGet),
        m::PROFILES_CREATE => call!(ProfileInput, ProfilesCreate),
        m::PROFILES_UPDATE => call!(ProfileUpdateParams, ProfilesUpdate),
        m::PROFILES_DELETE => call!(IdParams, ProfilesDelete),
        m::ENTRIES_LIST => call!(ListEntriesParams, EntriesList),
        m::ENTRIES_GET => call!(GetEntryParams, EntriesGet),
        m::ENTRIES_REVEAL => call!(RevealParams, EntriesReveal),
        m::ENTRIES_CREATE => call!(EntryInput, EntriesCreate),
        m::ENTRIES_UPDATE => call!(EntryUpdateParams, EntriesUpdate),
        m::ENTRIES_DELETE => call!(IdParams, EntriesDelete),
        m::ENTRIES_MOVE => call!(MoveEntryParams, EntriesMove),
        m::ENTRIES_HISTORY => call!(IdParams, EntriesHistory),
        m::ENTRIES_RESTORE => call!(RestoreParams, EntriesRestore),
        m::ENTRIES_TRASH => call!(TrashParams, EntriesTrash),
        m::ACTIONS_LIST => call!(IdParams, ActionsList),
        m::ACTIONS_SAVE => call!(ActionSaveParams, ActionsSave),
        m::ACTIONS_DELETE => call!(ActionIdParams, ActionsDelete),
        m::ACTIONS_CONFIGURE => call!(ActionConfigureParams, ActionsConfigure),
        m::ACTIONS_RUN => call!(ActionIdParams, ActionsRun),
        m::ENV_RESOLVE => call!(EnvResolveParams, EnvResolve),
        m::ENV_FORMAT => call!(EnvFormatParams, EnvFormat),
        m::ENV_PARSE => call!(EnvParseParams, EnvParse),
        m::ENV_COPY => call!(EnvFormatParams, EnvCopy),
        m::TOTP_GET => call!(IdParams, TotpGet),
        m::TOTP_PINNED => Call::TotpPinned(empty(method, params)?),
        m::CLIPBOARD_COPY => call!(ClipboardCopyParams, ClipboardCopy),
        m::BROWSER_LAUNCH => call!(BrowserLaunchParams, BrowserLaunch),
        m::BROWSER_LIST => Call::BrowserList(empty(method, params)?),
        m::BROWSER_CLOSE => call!(IdParams, BrowserClose),
        m::BROWSER_RESET => call!(IdParams, BrowserReset),
        m::BROWSER_DETECT => Call::BrowserDetect(empty(method, params)?),
        m::BROWSER_SESSION_INFO => call!(IdParams, BrowserSessionInfo),
        m::SYNC_STATUS => Call::SyncStatus(empty(method, params)?),
        m::SYNC_RUN => call!(SyncParams, SyncRun),
        m::SYNC_CONFLICT => call!(IdParams, SyncConflict),
        m::SYNC_RESOLVE => call!(ResolveParams, SyncResolve),
        m::SYNC_SET_REMOTE => call!(SetRemoteParams, SyncSetRemote),
        m::RECIPIENTS_LIST => Call::RecipientsList(empty(method, params)?),
        m::RECIPIENTS_ADD => call!(AddRecipientParams, RecipientsAdd),
        m::RECIPIENTS_REMOVE => call!(RemoveRecipientParams, RecipientsRemove),
        m::RECIPIENTS_ROTATE => Call::RecipientsRotate(empty(method, params)?),
        m::IMPORT_PREVIEW => call!(ImportParams, ImportPreview),
        m::IMPORT_APPLY => call!(ImportParams, ImportApply),
        m::EXPORT_RUN => call!(ExportParams, ExportRun),
        m::DOCTOR_RUN => call!(DoctorParams, DoctorRun),
        m::SETTINGS_GET => Call::SettingsGet(empty(method, params)?),
        m::SETTINGS_UPDATE => call!(Settings, SettingsUpdate),
        m::VAULT_LOCATE => call!(OpenParams, VaultLocate),
        m::DOCTOR_SCAN => call!(OpenParams, DoctorScan),
        m::HOOKS_INSTALL => call!(HookParams, HooksInstall),
        m::HOOKS_UNINSTALL => call!(HookParams, HooksUninstall),
        m::TOTP_PARSE => call!(EnvParseParams, TotpParse),
        _ => {
            return Err(RpcError {
                code: ErrorCode::InvalidParams,
                message: format!("invalid parameters: unknown method `{method}`"),
            });
        }
    };
    Ok(call)
}

pub fn test_reply(reply: Reply) -> Result<Value, RpcError> {
    macro_rules! value {
        ($value:expr) => {
            serde_json::to_value($value).map_err(|_| RpcError {
                code: ErrorCode::Internal,
                message: "test reply serialization failed".into(),
            })?
        };
    }
    Ok(match reply {
        Reply::DomainCall(v) => value!(DomainResponse::try_from(v).map_err(|_| RpcError {
            code: ErrorCode::InvalidParams,
            message: "invalid reply".into()
        })?),
        Reply::Hello(v) => value!(HelloResult::try_from(v).map_err(|_| bad_reply())?),
        Reply::VaultStatus(v) => value!(VaultStatus::try_from(v).map_err(|_| bad_reply())?),
        Reply::VaultInit(v) => value!(InitResult::try_from(v).map_err(|_| bad_reply())?),
        Reply::VaultClone(v)
        | Reply::VaultOpen(v)
        | Reply::VaultUnlock(v)
        | Reply::VaultLock(v) => value!(VaultStatus::try_from(v).map_err(|_| bad_reply())?),
        Reply::VaultList(v) => value!(
            v.items
                .into_iter()
                .map(KnownVault::try_from)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| bad_reply())?
        ),
        Reply::VaultBenchmarkKdf(v) => value!(KdfBenchmark::try_from(v).map_err(|_| bad_reply())?),
        Reply::VaultChangePassphrase(_)
        | Reply::ProfilesDelete(_)
        | Reply::EntriesDelete(_)
        | Reply::ActionsDelete(_)
        | Reply::ActionsConfigure(_)
        | Reply::ActionsRun(_)
        | Reply::BrowserClose(_)
        | Reply::BrowserReset(_) => Value::Null,
        Reply::VaultTrust(v) | Reply::VaultForgetTrust(v) => {
            value!(TrustStatus::try_from(v).map_err(|_| bad_reply())?)
        }
        Reply::ProfilesList(v) => value!(
            v.items
                .into_iter()
                .map(ProfileSummary::try_from)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| bad_reply())?
        ),
        Reply::ProfilesGet(v) | Reply::ProfilesCreate(v) | Reply::ProfilesUpdate(v) => {
            value!(Profile::try_from(v).map_err(|_| bad_reply())?)
        }
        Reply::EntriesList(v) => value!(
            v.items
                .into_iter()
                .map(EntrySummary::try_from)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| bad_reply())?
        ),
        Reply::EntriesGet(v)
        | Reply::EntriesCreate(v)
        | Reply::EntriesUpdate(v)
        | Reply::EntriesMove(v)
        | Reply::EntriesRestore(v) => value!(Entry::try_from(v).map_err(|_| bad_reply())?),
        Reply::EntriesReveal(v) => value!(RevealResult::try_from(v).map_err(|_| bad_reply())?),
        Reply::EntriesHistory(v) => value!(
            v.items
                .into_iter()
                .map(HistoryItem::try_from)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| bad_reply())?
        ),
        Reply::EntriesTrash(v) => value!(
            v.items
                .into_iter()
                .map(TrashItem::try_from)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| bad_reply())?
        ),
        Reply::ActionsList(v) => value!(ActionListResult::try_from(v).map_err(|_| bad_reply())?),
        Reply::ActionsSave(v) => value!(LaunchAction::try_from(v).map_err(|_| bad_reply())?),
        Reply::EnvResolve(v) => value!(EnvResolveResult::try_from(v).map_err(|_| bad_reply())?),
        Reply::EnvFormat(v) => value!(TextResult::try_from(v).map_err(|_| bad_reply())?),
        Reply::EnvParse(v) => value!(
            v.items
                .into_iter()
                .map(EnvVar::try_from)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| bad_reply())?
        ),
        Reply::EnvCopy(v) | Reply::ClipboardCopy(v) => {
            value!(ClipboardCopyResult::try_from(v).map_err(|_| bad_reply())?)
        }
        Reply::TotpGet(v) => value!(TotpCode::try_from(v).map_err(|_| bad_reply())?),
        Reply::TotpPinned(v) => value!(
            v.items
                .into_iter()
                .map(PinnedTotp::try_from)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| bad_reply())?
        ),
        Reply::BrowserLaunch(v) => value!(BrowserSession::try_from(v).map_err(|_| bad_reply())?),
        Reply::BrowserList(v) => value!(
            v.items
                .into_iter()
                .map(BrowserSession::try_from)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| bad_reply())?
        ),
        Reply::BrowserDetect(v) => value!(
            v.items
                .into_iter()
                .map(DetectedBrowser::try_from)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| bad_reply())?
        ),
        Reply::BrowserSessionInfo(v) => {
            value!(SessionArchiveInfo::try_from(v).map_err(|_| bad_reply())?)
        }
        Reply::SyncStatus(v) | Reply::SyncResolve(v) | Reply::SyncSetRemote(v) => {
            value!(SyncStatus::try_from(v).map_err(|_| bad_reply())?)
        }
        Reply::SyncRun(v) => value!(SyncResult::try_from(v).map_err(|_| bad_reply())?),
        Reply::SyncConflict(v) => value!(ConflictDetail::try_from(v).map_err(|_| bad_reply())?),
        Reply::RecipientsList(v) => value!(
            v.items
                .into_iter()
                .map(RecipientInfo::try_from)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| bad_reply())?
        ),
        Reply::RecipientsAdd(v) => value!(
            v.items
                .into_iter()
                .map(RecipientInfo::try_from)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| bad_reply())?
        ),
        Reply::RecipientsRemove(v) => value!(
            v.items
                .into_iter()
                .map(RecipientInfo::try_from)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| bad_reply())?
        ),
        Reply::RecipientsRotate(v) => value!(
            v.items
                .into_iter()
                .map(RecipientInfo::try_from)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| bad_reply())?
        ),
        Reply::ImportPreview(v) => value!(ImportPreview::try_from(v).map_err(|_| bad_reply())?),
        Reply::ImportApply(v) => value!(ImportApplyResult::try_from(v).map_err(|_| bad_reply())?),
        Reply::ExportRun(v) => value!(ExportResult::try_from(v).map_err(|_| bad_reply())?),
        Reply::DoctorRun(v) | Reply::DoctorScan(v) => {
            value!(DoctorReport::try_from(v).map_err(|_| bad_reply())?)
        }
        Reply::SettingsGet(v) | Reply::SettingsUpdate(v) => {
            value!(Settings::try_from(v).map_err(|_| bad_reply())?)
        }
        Reply::VaultLocate(v) => value!(TextResult::try_from(v).map_err(|_| bad_reply())?),
        Reply::HooksInstall(v) | Reply::HooksUninstall(v) => {
            value!(HookResult::try_from(v).map_err(|_| bad_reply())?)
        }
        Reply::TotpParse(v) => value!(TotpConfig::try_from(v).map_err(|_| bad_reply())?),
    })
}

fn bad_reply() -> RpcError {
    RpcError {
        code: ErrorCode::Internal,
        message: "invalid typed reply in test adapter".into(),
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
            start_pages: vec![],
            bookmarks: vec![],
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
