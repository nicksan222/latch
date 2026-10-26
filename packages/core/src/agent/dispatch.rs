//! Method routing: `RpcRequest` → core call → `RpcResponse`.

use std::path::PathBuf;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use zeroize::Zeroizing;

use super::{Agent, ClientCtx, state};
use crate::core::{Error, Result, UnlockedVault, Vault, crypto};
use crate::protocol::methods as m;
use crate::protocol::*;

mod local;
mod vault;

type R = Result<Value>;

fn ok<T: Serialize>(v: T) -> R {
    serde_json::to_value(v).map_err(|e| Error::Internal(format!("serialize: {e}")))
}

/// Parse params without echoing input values in errors (serde messages can
/// quote them).
fn params<T: DeserializeOwned>(method: &str, v: Value) -> Result<T> {
    let v = if v.is_null() {
        Value::Object(Default::default())
    } else {
        v
    };
    serde_json::from_value(v).map_err(|e| {
        let s = e.to_string();
        if s.starts_with("missing field") {
            let field = s.split('`').nth(1).unwrap_or("?");
            Error::InvalidParams(format!("{method}: missing field `{field}`"))
        } else {
            Error::InvalidParams(format!("{method}: malformed parameters"))
        }
    })
}

fn rpc_ok(id: u64, v: Value) -> RpcResponse {
    RpcResponse {
        id,
        result: Some(v),
        error: None,
    }
}

pub(crate) fn rpc_err(id: u64, e: RpcError) -> RpcResponse {
    RpcResponse {
        id,
        result: None,
        error: Some(e),
    }
}

pub(crate) fn problem_strings(v: &UnlockedVault) -> Vec<String> {
    v.integrity_problems()
        .iter()
        .map(|p| match &p.path {
            Some(path) => format!("{}: {}", path, p.message),
            None => p.message.clone(),
        })
        .collect()
}

fn denied() -> Error {
    Error::Denied("not permitted for this client".into())
}

impl Agent {
    /// First message of a CLI or desktop connection.
    pub fn hello(&self, p: HelloParams) -> std::result::Result<(ClientCtx, HelloResult), RpcError> {
        let ctx = ClientCtx { kind: p.client };
        Ok((
            ctx,
            HelloResult {
                agent_version: env!("CARGO_PKG_VERSION").into(),
                protocol: PROTOCOL_VERSION,
            },
        ))
    }

    /// Serve one request. Synchronous; may block on the KDF or git.
    pub fn dispatch(&self, ctx: &ClientCtx, req: RpcRequest) -> RpcResponse {
        let id = req.id;
        let method = req.method.clone();
        // Passive reads (polled by the UI) do not reset the idle timer.
        let passive = matches!(
            method.as_str(),
            m::HELLO | m::VAULT_STATUS | m::SYNC_STATUS | m::TOTP_GET | m::SETTINGS_GET
        );
        self.touch(!passive);
        match self.route(ctx, &method, req.params) {
            Ok(v) => rpc_ok(id, v),
            Err(e) => {
                tracing::debug!(method = %method, code = ?e.code(), "rpc error");
                rpc_err(id, e.to_rpc())
            }
        }
    }

    fn route(&self, ctx: &ClientCtx, method: &str, p: Value) -> R {
        match method {
            m::DOMAIN_CALL => ok(crate::core::domain::call(params(method, p)?)?),
            m::HELLO => {
                let hp: HelloParams = params(method, p)?;
                if hp.client != ctx.kind {
                    return Err(denied());
                }
                ok(HelloResult {
                    agent_version: env!("CARGO_PKG_VERSION").into(),
                    protocol: PROTOCOL_VERSION,
                })
            }
            // ----- vault
            m::VAULT_STATUS => ok(self.vault_status()),
            m::VAULT_INIT => self.vault_init(params(method, p)?),
            m::VAULT_CLONE => {
                let cp: CloneParams = params(method, p)?;
                let v = Vault::clone_from(&cp.url, &cp.path)?;
                self.activate(v)
            }
            m::VAULT_OPEN => {
                let op: OpenParams = params(method, p)?;
                let v = Vault::open(&op.path)?;
                self.activate(v)
            }
            m::VAULT_LOCATE => ok(local::vault_locate(self, params(method, p)?)?),
            m::VAULT_LIST => ok(state::VaultsFile::load(&self.config_dir).known()),
            m::VAULT_UNLOCK => self.vault_unlock(params(method, p)?),
            m::VAULT_LOCK => {
                self.lock();
                ok(self.vault_status())
            }
            m::VAULT_TRUST => ok(self.trust_status()?),
            m::VAULT_FORGET_TRUST => self.forget_trust(params(method, p)?),
            m::VAULT_BENCHMARK_KDF => {
                let bp: KdfBenchmarkParams = params(method, p)?;
                ok(crypto::calibrate_kdf(bp.target_ms)?)
            }
            m::VAULT_CHANGE_PASSPHRASE => {
                let cp: ChangePassphraseParams = params(method, p)?;
                self.write(|v| v.change_passphrase(&cp))?;
                self.emit(AgentEventKind::VaultChanged, None, None, None);
                ok(Value::Null)
            }
            // ----- profiles
            m::PROFILES_LIST => {
                let mut list = self.read(|v| v.list_profiles())?;
                for p in &mut list {
                    p.browser_running = self.browsers().is_running(&p.id);
                }
                ok(list)
            }
            m::PROFILES_GET => {
                let ip: IdParams = params(method, p)?;
                ok(self.read(|v| v.get_profile(&ip.id))?)
            }
            m::PROFILES_CREATE => {
                let ip: ProfileInput = params(method, p)?;
                let r = self.write(|v| v.create_profile(&ip))?;
                self.emit(
                    AgentEventKind::ProfilesChanged,
                    Some(r.id.clone()),
                    None,
                    None,
                );
                ok(r)
            }
            m::PROFILES_UPDATE => {
                let up: ProfileUpdateParams = params(method, p)?;
                let r = self.write(|v| v.update_profile(&up))?;
                self.emit(
                    AgentEventKind::ProfilesChanged,
                    Some(r.id.clone()),
                    None,
                    None,
                );
                ok(r)
            }
            m::PROFILES_DELETE => {
                let ip: IdParams = params(method, p)?;
                if self.browsers().is_running(&ip.id) {
                    self.browsers().close(&ip.id)?;
                }
                self.write(|v| v.delete_profile(&ip.id))?;
                self.emit(AgentEventKind::ProfilesChanged, Some(ip.id), None, None);
                ok(Value::Null)
            }
            // ----- entries
            m::ENTRIES_LIST => {
                let lp: ListEntriesParams = params(method, p)?;
                ok(self.read(|v| v.list_entries(&lp))?)
            }
            m::ENTRIES_GET => {
                let gp: GetEntryParams = params(method, p)?;
                ok(self.read(|v| v.get_entry(&gp))?)
            }
            m::ENTRIES_REVEAL => {
                let rp: RevealParams = params(method, p)?;
                ok(self.read(|v| v.reveal(&rp))?)
            }
            m::ENTRIES_CREATE => {
                let ip: EntryInput = params(method, p)?;
                let e = self.write(|v| v.create_entry(&ip))?;
                self.entry_changed(&e);
                ok(e)
            }
            m::ENTRIES_UPDATE => {
                let up: EntryUpdateParams = params(method, p)?;
                let e = self.write(|v| v.update_entry(&up))?;
                self.entry_changed(&e);
                ok(e)
            }
            m::ENTRIES_DELETE => {
                let ip: IdParams = params(method, p)?;
                self.write(|v| v.delete_entry(&ip.id))?;
                self.emit(AgentEventKind::EntriesChanged, None, Some(ip.id), None);
                ok(Value::Null)
            }
            m::ENTRIES_MOVE => {
                let mp: MoveEntryParams = params(method, p)?;
                let e = self.write(|v| v.move_entry(&mp))?;
                self.entry_changed(&e);
                ok(e)
            }
            m::ENTRIES_HISTORY => {
                let ip: IdParams = params(method, p)?;
                ok(self.read(|v| v.history(&ip.id))?)
            }
            m::ENTRIES_RESTORE => {
                let rp: RestoreParams = params(method, p)?;
                let e = self.write(|v| v.restore(&rp))?;
                self.entry_changed(&e);
                ok(e)
            }
            m::ENTRIES_TRASH => {
                let tp: TrashParams = params(method, p)?;
                ok(self.read(|v| v.trash(&tp))?)
            }
            // ----- saved project launches
            m::ACTIONS_LIST => {
                let ip: IdParams = params(method, p)?;
                ok(self.actions_list(&ip.id)?)
            }
            m::ACTIONS_SAVE => {
                let ap: ActionSaveParams = params(method, p)?;
                let action = self.write(|v| v.save_action(&ap))?;
                self.emit(
                    AgentEventKind::ProfilesChanged,
                    Some(ap.profile_id),
                    None,
                    None,
                );
                ok(action)
            }
            m::ACTIONS_DELETE => {
                let ap: ActionIdParams = params(method, p)?;
                self.write(|v| v.delete_action(&ap))?;
                self.emit(
                    AgentEventKind::ProfilesChanged,
                    Some(ap.profile_id),
                    None,
                    None,
                );
                ok(())
            }
            m::ACTIONS_CONFIGURE => {
                let ap: ActionConfigureParams = params(method, p)?;
                self.actions_configure(&ap)?;
                self.emit(
                    AgentEventKind::ProfilesChanged,
                    Some(ap.profile_id),
                    None,
                    None,
                );
                ok(())
            }
            m::ACTIONS_RUN => {
                let ap: ActionIdParams = params(method, p)?;
                self.actions_run(&ap)?;
                ok(())
            }
            // ----- env, totp, clipboard
            m::ENV_RESOLVE => {
                let ep: EnvResolveParams = params(method, p)?;
                ok(self.read(|v| v.env_resolve(&ep))?)
            }
            m::ENV_FORMAT => {
                let ep: EnvFormatParams = params(method, p)?;
                ok(self.read(|v| v.env_format(&ep))?)
            }
            m::ENV_PARSE => {
                let ep: EnvParseParams = params(method, p)?;
                ok(crate::core::environment::parse(&ep.text)?)
            }
            m::ENV_COPY => {
                let ep: EnvFormatParams = params(method, p)?;
                let text = Zeroizing::new(self.read(|v| v.env_format(&ep))?.text);
                let secs = self.settings_snapshot().clipboard_clear_secs;
                self.copy_to_clipboard(text, secs)?;
                ok(ClipboardCopyResult {
                    clear_after_secs: secs,
                })
            }
            m::TOTP_GET => {
                let ip: IdParams = params(method, p)?;
                ok(self.read(|v| v.totp(&ip.id))?)
            }
            m::TOTP_PINNED => ok(self.read(|v| v.pinned_totp())?),
            m::TOTP_PARSE => ok(local::totp_parse(params(method, p)?)?),
            m::CLIPBOARD_COPY => self.clipboard_copy(p),
            // ----- browser
            m::BROWSER_LAUNCH => {
                let bp: BrowserLaunchParams = params(method, p)?;
                ok(self.read(|v| self.browsers().launch(v, &bp))?)
            }
            m::BROWSER_LIST => ok(self.browsers().list()),
            m::BROWSER_CLOSE => {
                let ip: IdParams = params(method, p)?;
                self.browsers().close(&ip.id)?;
                ok(Value::Null)
            }
            m::BROWSER_RESET => {
                let ip: IdParams = params(method, p)?;
                self.read(|v| self.browsers().reset(v, &ip.id))?;
                ok(Value::Null)
            }
            m::BROWSER_DETECT => ok(super::browser::BrowserManager::detect()),
            m::BROWSER_SESSION_INFO => {
                let ip: IdParams = params(method, p)?;
                ok(self.read(|v| self.browsers().session_info(v, &ip.id))?)
            }
            // ----- sync
            m::SYNC_STATUS => ok(self.read(|v| v.sync_status())?),
            m::SYNC_RUN => {
                let sp: SyncParams = params(method, p)?;
                let r = self.write(|v| v.sync(&sp))?;
                self.emit(AgentEventKind::SyncChanged, None, None, None);
                if r.pulled > 0 {
                    self.emit(AgentEventKind::ProfilesChanged, None, None, None);
                    self.emit(AgentEventKind::EntriesChanged, None, None, None);
                }
                ok(r)
            }
            m::SYNC_CONFLICT => {
                let ip: IdParams = params(method, p)?;
                ok(self.read(|v| v.conflict_detail(&ip.id))?)
            }
            m::SYNC_RESOLVE => {
                let rp: ResolveParams = params(method, p)?;
                let r = self.write(|v| v.resolve(&rp))?;
                self.emit(AgentEventKind::SyncChanged, None, None, None);
                self.emit(
                    AgentEventKind::EntriesChanged,
                    None,
                    Some(rp.entry_id),
                    None,
                );
                ok(r)
            }
            m::SYNC_SET_REMOTE => {
                let sp: SetRemoteParams = params(method, p)?;
                let r = self.write(|v| v.set_remote(&sp.url))?;
                self.emit(AgentEventKind::SyncChanged, None, None, None);
                ok(r)
            }
            // ----- recipients
            m::RECIPIENTS_LIST => ok(self.read(|v| Ok(v.list_recipients()))?),
            m::RECIPIENTS_ADD => {
                let ap: AddRecipientParams = params(method, p)?;
                let r = self.write(|v| v.add_recipient(&ap))?;
                self.emit(AgentEventKind::VaultChanged, None, None, None);
                ok(r)
            }
            m::RECIPIENTS_REMOVE => {
                let rp: RemoveRecipientParams = params(method, p)?;
                let r = self.write(|v| v.remove_recipient(&rp))?;
                self.emit(AgentEventKind::VaultChanged, None, None, None);
                ok(r)
            }
            m::RECIPIENTS_ROTATE => {
                let r = self.write(|v| v.rotate())?;
                self.emit(AgentEventKind::VaultChanged, None, None, None);
                ok(r)
            }
            // ----- import / export / doctor
            m::IMPORT_PREVIEW => {
                let ip: ImportParams = params(method, p)?;
                ok(self.read(|v| crate::core::import::preview(v, &ip))?)
            }
            m::IMPORT_APPLY => {
                let ip: ImportParams = params(method, p)?;
                let r = self.write(|v| crate::core::import::apply(v, &ip))?;
                self.emit(AgentEventKind::ProfilesChanged, None, None, None);
                self.emit(AgentEventKind::EntriesChanged, None, None, None);
                ok(r)
            }
            m::EXPORT_RUN => {
                let ep: ExportParams = params(method, p)?;
                ok(self.read(|v| crate::core::export::run(v, &ep))?)
            }
            m::DOCTOR_RUN => self.doctor(params(method, p)?),
            m::DOCTOR_SCAN => ok(local::doctor_scan(params(method, p)?)?),
            m::HOOKS_INSTALL => ok(local::hooks_install(params(method, p)?)?),
            m::HOOKS_UNINSTALL => ok(local::hooks_uninstall(params(method, p)?)?),
            // ----- settings
            m::SETTINGS_GET => ok(self.settings_get()),
            m::SETTINGS_UPDATE => self.settings_update(params(method, p)?),
            _ => Err(Error::InvalidParams(format!("unknown method `{method}`"))),
        }
    }

    // ----- vault slot access ----------------------------------------------

    fn no_vault_or_locked(path: &Option<PathBuf>) -> Error {
        match path {
            None => Error::NoVault("(none configured; run `latch init`)".into()),
            Some(_) => Error::Locked,
        }
    }

    /// Run `f` on the unlocked vault (shared lock).
    pub(crate) fn read<T>(&self, f: impl FnOnce(&UnlockedVault) -> Result<T>) -> Result<T> {
        let s = self.slot.read().unwrap_or_else(|p| p.into_inner());
        match &s.unlocked {
            Some(v) => f(v),
            None => Err(Self::no_vault_or_locked(&s.path)),
        }
    }

    /// Run `f` on the unlocked vault (exclusive lock: writes serialized).
    pub(crate) fn write<T>(&self, f: impl FnOnce(&mut UnlockedVault) -> Result<T>) -> Result<T> {
        let mut s = self.slot.write().unwrap_or_else(|p| p.into_inner());
        let s = &mut *s;
        match &mut s.unlocked {
            Some(v) => {
                let result = f(v)?;
                self.browsers.refresh_credentials(v);
                Ok(result)
            }
            None => Err(Self::no_vault_or_locked(&s.path)),
        }
    }

    fn entry_changed(&self, e: &Entry) {
        self.emit(
            AgentEventKind::EntriesChanged,
            Some(e.profile_id.clone()),
            Some(e.id.clone()),
            None,
        );
    }

    // ----- settings ---------------------------------------------------------

    pub(crate) fn settings_get(&self) -> Settings {
        let mut s = self.settings_snapshot();
        let slot = self.slot.read().unwrap_or_else(|p| p.into_inner());
        // Effective values: config.toml flag AND this clone's consent.
        let eff = match (&slot.unlocked, &slot.path) {
            (Some(v), _) => Some(v.effective_settings()),
            (None, Some(p)) => Vault::open(p).ok().map(|v| {
                let c = crate::core::trust::consent(v.id(), v.path());
                crate::core::Consent {
                    names_in_commits: v.config().sync.names_in_commits && c.names_in_commits,
                    auto_push: v.config().sync.auto_push && c.auto_push,
                    sessions_commit: v.config().sessions.commit && c.sessions_commit,
                }
            }),
            _ => None,
        };
        if let Some(c) = eff {
            s.names_in_commits = c.names_in_commits;
            s.auto_push = c.auto_push;
            s.sessions_commit = c.sessions_commit;
        }
        s
    }

    fn settings_update(&self, new: Settings) -> R {
        state::validate_settings(&new)?;
        if !self.is_unlocked() {
            let path = self
                .slot
                .read()
                .unwrap_or_else(|p| p.into_inner())
                .path
                .clone();
            return Err(Self::no_vault_or_locked(&path));
        }
        let current = self.settings_get();
        let vault_flags_changed = current.names_in_commits != new.names_in_commits
            || current.auto_push != new.auto_push
            || current.sessions_commit != new.sessions_commit;
        if vault_flags_changed {
            self.write(|v| {
                v.set_vault_settings(new.names_in_commits, new.auto_push, new.sessions_commit)
            })?;
        }
        state::save_settings(&self.config_dir, &new)?;
        self.set_machine_settings(new);
        if self.is_unlocked() {
            let _ = self.read(|v| {
                self.apply_git_identity(v);
                Ok(())
            });
        }
        self.emit(AgentEventKind::VaultChanged, None, None, None);
        ok(self.settings_get())
    }

    // ----- clipboard --------------------------------------------------------

    fn clipboard_copy(&self, p: Value) -> R {
        let cp: ClipboardCopyParams = params(m::CLIPBOARD_COPY, p)?;
        // The procedure contract includes the per-copy CLI/UI override. Keep
        // the existing full wire range and cap the effective timer at one day.
        let override_secs = cp.clear_after_secs.map(|v| v.min(24 * 3600) as u32);
        let value: Zeroizing<String> = self.read(|v| match cp.field.as_str() {
            "totp" => Ok(Zeroizing::new(v.totp(&cp.id)?.code)),
            field => Ok(Zeroizing::new(
                v.reveal(&RevealParams {
                    id: cp.id.clone(),
                    field: field.to_owned(),
                })?
                .value,
            )),
        })?;
        let secs = override_secs.unwrap_or_else(|| self.settings_snapshot().clipboard_clear_secs);
        self.copy_to_clipboard(value, secs)?;
        ok(ClipboardCopyResult {
            clear_after_secs: secs,
        })
    }
}
