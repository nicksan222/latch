//! Typed procedure routing into core domain operations.
use super::{Agent, ClientCtx, state};
use crate::core::{Error, Result, UnlockedVault, Vault, crypto};
use crate::protocol::*;
use latch_rpc::{
    sdk::{Call, Reply},
    v1,
};
use std::path::PathBuf;
use zeroize::Zeroizing;
mod local;
mod vault;

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

    /// Execute one typed procedure. Synchronous; may block on the KDF or git.
    pub fn execute(&self, ctx: &ClientCtx, call: Call) -> std::result::Result<Reply, RpcError> {
        let passive = matches!(
            &call,
            Call::Hello(_)
                | Call::VaultStatus(_)
                | Call::SyncStatus(_)
                | Call::TotpGet(_)
                | Call::SettingsGet(_)
        );
        self.touch(!passive);
        self.route(ctx, call).map_err(|error| {
            tracing::debug!(code = ?error.code(), "rpc error");
            error.to_rpc()
        })
    }

    fn route(&self, ctx: &ClientCtx, call: Call) -> Result<Reply> {
        match call {
            Call::DomainCall(p) => {
                let p: DomainRequest = p.try_into()?;
                let result: Result<DomainResponse> = { Ok(crate::core::domain::call(p)?) };
                result.map(|value| Reply::DomainCall(value.into()))
            }
            Call::Hello(p) => {
                let p: HelloParams = p.try_into()?;
                let result: Result<HelloResult> = {
                    let hp: HelloParams = p;
                    if hp.client != ctx.kind {
                        return Err(denied());
                    }
                    Ok(HelloResult {
                        agent_version: env!("CARGO_PKG_VERSION").into(),
                        protocol: PROTOCOL_VERSION,
                    })
                };
                result.map(|value| Reply::Hello(value.into()))
            }
            Call::VaultStatus(_) => {
                let result: Result<VaultStatus> = { Ok(self.vault_status()) };
                result.map(|value| Reply::VaultStatus(value.into()))
            }
            Call::VaultInit(p) => {
                let p: InitParams = p.try_into()?;
                let result: Result<InitResult> = { self.vault_init(p) };
                result.map(|value| Reply::VaultInit(value.into()))
            }
            Call::VaultClone(p) => {
                let p: CloneParams = p.try_into()?;
                let result: Result<VaultStatus> = {
                    let cp: CloneParams = p;
                    let v = Vault::clone_from(&cp.url, &cp.path)?;
                    self.activate(v)
                };
                result.map(|value| Reply::VaultClone(value.into()))
            }
            Call::VaultOpen(p) => {
                let p: OpenParams = p.try_into()?;
                let result: Result<VaultStatus> = {
                    let op: OpenParams = p;
                    let v = Vault::open(&op.path)?;
                    self.activate(v)
                };
                result.map(|value| Reply::VaultOpen(value.into()))
            }
            Call::VaultList(_) => {
                let result: Result<Vec<KnownVault>> =
                    { Ok(state::VaultsFile::load(&self.config_dir).known()) };
                result.map(|value| {
                    Reply::VaultList(v1::VaultListResponse {
                        items: value.into_iter().map(Into::into).collect(),
                    })
                })
            }
            Call::VaultUnlock(p) => {
                let p: UnlockParams = p.try_into()?;
                let result: Result<VaultStatus> = { self.vault_unlock(p) };
                result.map(|value| Reply::VaultUnlock(value.into()))
            }
            Call::VaultLock(_) => {
                let result: Result<VaultStatus> = {
                    self.lock();
                    Ok(self.vault_status())
                };
                result.map(|value| Reply::VaultLock(value.into()))
            }
            Call::VaultBenchmarkKdf(p) => {
                let p: KdfBenchmarkParams = p.try_into()?;
                let result: Result<KdfBenchmark> = {
                    let bp: KdfBenchmarkParams = p;
                    Ok(crypto::calibrate_kdf(bp.target_ms)?)
                };
                result.map(|value| Reply::VaultBenchmarkKdf(value.into()))
            }
            Call::VaultChangePassphrase(p) => {
                let p: ChangePassphraseParams = p.try_into()?;
                let result: Result<()> = {
                    let cp: ChangePassphraseParams = p;
                    self.write(|v| v.change_passphrase(&cp))?;
                    self.emit(AgentEventKind::VaultChanged, None, None, None);
                    Ok(())
                };
                result.map(|()| Reply::VaultChangePassphrase(v1::Empty {}))
            }
            Call::VaultTrust(_) => {
                let result: Result<TrustStatus> = { Ok(self.trust_status()?) };
                result.map(|value| Reply::VaultTrust(value.into()))
            }
            Call::VaultForgetTrust(p) => {
                let p: ForgetTrustParams = p.try_into()?;
                let result: Result<TrustStatus> = { self.forget_trust(p) };
                result.map(|value| Reply::VaultForgetTrust(value.into()))
            }
            Call::ProfilesList(_) => {
                let result: Result<Vec<ProfileSummary>> = {
                    let mut list = self.read(|v| v.list_profiles())?;
                    for p in &mut list {
                        p.browser_running = self.browsers().is_running(&p.id);
                    }
                    Ok(list)
                };
                result.map(|value| {
                    Reply::ProfilesList(v1::ProfilesListResponse {
                        items: value.into_iter().map(Into::into).collect(),
                    })
                })
            }
            Call::ProfilesGet(p) => {
                let p: IdParams = p.try_into()?;
                let result: Result<Profile> = {
                    let ip: IdParams = p;
                    Ok(self.read(|v| v.get_profile(&ip.id))?)
                };
                result.map(|value| Reply::ProfilesGet(value.into()))
            }
            Call::ProfilesCreate(p) => {
                let p: ProfileInput = p.try_into()?;
                let result: Result<Profile> = {
                    let ip: ProfileInput = p;
                    let r = self.write(|v| v.create_profile(&ip))?;
                    self.emit(
                        AgentEventKind::ProfilesChanged,
                        Some(r.id.clone()),
                        None,
                        None,
                    );
                    Ok(r)
                };
                result.map(|value| Reply::ProfilesCreate(value.into()))
            }
            Call::ProfilesUpdate(p) => {
                let p: ProfileUpdateParams = p.try_into()?;
                let result: Result<Profile> = {
                    let up: ProfileUpdateParams = p;
                    let r = self.write(|v| v.update_profile(&up))?;
                    self.emit(
                        AgentEventKind::ProfilesChanged,
                        Some(r.id.clone()),
                        None,
                        None,
                    );
                    Ok(r)
                };
                result.map(|value| Reply::ProfilesUpdate(value.into()))
            }
            Call::ProfilesDelete(p) => {
                let p: IdParams = p.try_into()?;
                let result: Result<()> = {
                    let ip: IdParams = p;
                    if self.browsers().is_running(&ip.id) {
                        self.browsers().close(&ip.id)?;
                    }
                    self.write(|v| v.delete_profile(&ip.id))?;
                    self.emit(AgentEventKind::ProfilesChanged, Some(ip.id), None, None);
                    Ok(())
                };
                result.map(|()| Reply::ProfilesDelete(v1::Empty {}))
            }
            Call::EntriesList(p) => {
                let p: ListEntriesParams = p.try_into()?;
                let result: Result<Vec<EntrySummary>> = {
                    let lp: ListEntriesParams = p;
                    Ok(self.read(|v| v.list_entries(&lp))?)
                };
                result.map(|value| {
                    Reply::EntriesList(v1::EntriesListResponse {
                        items: value.into_iter().map(Into::into).collect(),
                    })
                })
            }
            Call::EntriesGet(p) => {
                let p: GetEntryParams = p.try_into()?;
                let result: Result<Entry> = {
                    let gp: GetEntryParams = p;
                    Ok(self.read(|v| v.get_entry(&gp))?)
                };
                result.map(|value| Reply::EntriesGet(value.into()))
            }
            Call::EntriesReveal(p) => {
                let p: RevealParams = p.try_into()?;
                let result: Result<RevealResult> = {
                    let rp: RevealParams = p;
                    Ok(self.read(|v| v.reveal(&rp))?)
                };
                result.map(|value| Reply::EntriesReveal(value.into()))
            }
            Call::EntriesCreate(p) => {
                let p: EntryInput = p.try_into()?;
                let result: Result<Entry> = {
                    let ip: EntryInput = p;
                    let e = self.write(|v| v.create_entry(&ip))?;
                    self.entry_changed(&e);
                    Ok(e)
                };
                result.map(|value| Reply::EntriesCreate(value.into()))
            }
            Call::EntriesUpdate(p) => {
                let p: EntryUpdateParams = p.try_into()?;
                let result: Result<Entry> = {
                    let up: EntryUpdateParams = p;
                    let e = self.write(|v| v.update_entry(&up))?;
                    self.entry_changed(&e);
                    Ok(e)
                };
                result.map(|value| Reply::EntriesUpdate(value.into()))
            }
            Call::EntriesDelete(p) => {
                let p: IdParams = p.try_into()?;
                let result: Result<()> = {
                    let ip: IdParams = p;
                    self.write(|v| v.delete_entry(&ip.id))?;
                    self.emit(AgentEventKind::EntriesChanged, None, Some(ip.id), None);
                    Ok(())
                };
                result.map(|()| Reply::EntriesDelete(v1::Empty {}))
            }
            Call::EntriesMove(p) => {
                let p: MoveEntryParams = p.try_into()?;
                let result: Result<Entry> = {
                    let mp: MoveEntryParams = p;
                    let e = self.write(|v| v.move_entry(&mp))?;
                    self.entry_changed(&e);
                    Ok(e)
                };
                result.map(|value| Reply::EntriesMove(value.into()))
            }
            Call::EntriesHistory(p) => {
                let p: IdParams = p.try_into()?;
                let result: Result<Vec<HistoryItem>> = {
                    let ip: IdParams = p;
                    Ok(self.read(|v| v.history(&ip.id))?)
                };
                result.map(|value| {
                    Reply::EntriesHistory(v1::EntriesHistoryResponse {
                        items: value.into_iter().map(Into::into).collect(),
                    })
                })
            }
            Call::EntriesRestore(p) => {
                let p: RestoreParams = p.try_into()?;
                let result: Result<Entry> = {
                    let rp: RestoreParams = p;
                    let e = self.write(|v| v.restore(&rp))?;
                    self.entry_changed(&e);
                    Ok(e)
                };
                result.map(|value| Reply::EntriesRestore(value.into()))
            }
            Call::EntriesTrash(p) => {
                let p: TrashParams = p.try_into()?;
                let result: Result<Vec<TrashItem>> = {
                    let tp: TrashParams = p;
                    Ok(self.read(|v| v.trash(&tp))?)
                };
                result.map(|value| {
                    Reply::EntriesTrash(v1::EntriesTrashResponse {
                        items: value.into_iter().map(Into::into).collect(),
                    })
                })
            }
            Call::ActionsList(p) => {
                let p: IdParams = p.try_into()?;
                let result: Result<ActionListResult> = {
                    let ip: IdParams = p;
                    Ok(self.actions_list(&ip.id)?)
                };
                result.map(|value| Reply::ActionsList(value.into()))
            }
            Call::ActionsSave(p) => {
                let p: ActionSaveParams = p.try_into()?;
                let result: Result<LaunchAction> = {
                    let ap: ActionSaveParams = p;
                    let action = self.write(|v| v.save_action(&ap))?;
                    self.emit(
                        AgentEventKind::ProfilesChanged,
                        Some(ap.profile_id),
                        None,
                        None,
                    );
                    Ok(action)
                };
                result.map(|value| Reply::ActionsSave(value.into()))
            }
            Call::ActionsDelete(p) => {
                let p: ActionIdParams = p.try_into()?;
                let result: Result<()> = {
                    let ap: ActionIdParams = p;
                    self.write(|v| v.delete_action(&ap))?;
                    self.emit(
                        AgentEventKind::ProfilesChanged,
                        Some(ap.profile_id),
                        None,
                        None,
                    );
                    Ok(())
                };
                result.map(|()| Reply::ActionsDelete(v1::Empty {}))
            }
            Call::ActionsConfigure(p) => {
                let p: ActionConfigureParams = p.try_into()?;
                let result: Result<()> = {
                    let ap: ActionConfigureParams = p;
                    self.actions_configure(&ap)?;
                    self.emit(
                        AgentEventKind::ProfilesChanged,
                        Some(ap.profile_id),
                        None,
                        None,
                    );
                    Ok(())
                };
                result.map(|()| Reply::ActionsConfigure(v1::Empty {}))
            }
            Call::ActionsRun(p) => {
                let p: ActionIdParams = p.try_into()?;
                let result: Result<()> = {
                    let ap: ActionIdParams = p;
                    self.actions_run(&ap)?;
                    Ok(())
                };
                result.map(|()| Reply::ActionsRun(v1::Empty {}))
            }
            Call::EnvResolve(p) => {
                let p: EnvResolveParams = p.try_into()?;
                let result: Result<EnvResolveResult> = {
                    let ep: EnvResolveParams = p;
                    Ok(self.read(|v| v.env_resolve(&ep))?)
                };
                result.map(|value| Reply::EnvResolve(value.into()))
            }
            Call::EnvFormat(p) => {
                let p: EnvFormatParams = p.try_into()?;
                let result: Result<TextResult> = {
                    let ep: EnvFormatParams = p;
                    Ok(self.read(|v| v.env_format(&ep))?)
                };
                result.map(|value| Reply::EnvFormat(value.into()))
            }
            Call::EnvParse(p) => {
                let p: EnvParseParams = p.try_into()?;
                let result: Result<Vec<EnvVar>> = {
                    let ep: EnvParseParams = p;
                    Ok(crate::core::environment::parse(&ep.text)?)
                };
                result.map(|value| {
                    Reply::EnvParse(v1::EnvParseResponse {
                        items: value.into_iter().map(Into::into).collect(),
                    })
                })
            }
            Call::EnvCopy(p) => {
                let p: EnvFormatParams = p.try_into()?;
                let result: Result<ClipboardCopyResult> = {
                    let ep: EnvFormatParams = p;
                    let text = Zeroizing::new(self.read(|v| v.env_format(&ep))?.text);
                    let secs = self.settings_snapshot().clipboard_clear_secs;
                    self.copy_to_clipboard(text, secs)?;
                    Ok(ClipboardCopyResult {
                        clear_after_secs: secs,
                    })
                };
                result.map(|value| Reply::EnvCopy(value.into()))
            }
            Call::TotpGet(p) => {
                let p: IdParams = p.try_into()?;
                let result: Result<TotpCode> = {
                    let ip: IdParams = p;
                    Ok(self.read(|v| v.totp(&ip.id))?)
                };
                result.map(|value| Reply::TotpGet(value.into()))
            }
            Call::TotpPinned(_) => {
                let result: Result<Vec<PinnedTotp>> = { Ok(self.read(|v| v.pinned_totp())?) };
                result.map(|value| {
                    Reply::TotpPinned(v1::TotpPinnedResponse {
                        items: value.into_iter().map(Into::into).collect(),
                    })
                })
            }
            Call::ClipboardCopy(p) => {
                let p: ClipboardCopyParams = p.try_into()?;
                let result: Result<ClipboardCopyResult> = { self.clipboard_copy(p) };
                result.map(|value| Reply::ClipboardCopy(value.into()))
            }
            Call::BrowserLaunch(p) => {
                let p: BrowserLaunchParams = p.try_into()?;
                let result: Result<BrowserSession> = {
                    let bp: BrowserLaunchParams = p;
                    Ok(self.read(|v| self.browsers().launch(v, &bp))?)
                };
                result.map(|value| Reply::BrowserLaunch(value.into()))
            }
            Call::BrowserList(_) => {
                let result: Result<Vec<BrowserSession>> = { Ok(self.browsers().list()) };
                result.map(|value| {
                    Reply::BrowserList(v1::BrowserListResponse {
                        items: value.into_iter().map(Into::into).collect(),
                    })
                })
            }
            Call::BrowserClose(p) => {
                let p: IdParams = p.try_into()?;
                let result: Result<()> = {
                    let ip: IdParams = p;
                    self.browsers().close(&ip.id)?;
                    Ok(())
                };
                result.map(|()| Reply::BrowserClose(v1::Empty {}))
            }
            Call::BrowserReset(p) => {
                let p: IdParams = p.try_into()?;
                let result: Result<()> = {
                    let ip: IdParams = p;
                    self.read(|v| self.browsers().reset(v, &ip.id))?;
                    Ok(())
                };
                result.map(|()| Reply::BrowserReset(v1::Empty {}))
            }
            Call::BrowserDetect(_) => {
                let result: Result<Vec<DetectedBrowser>> =
                    { Ok(super::browser::BrowserManager::detect()) };
                result.map(|value| {
                    Reply::BrowserDetect(v1::BrowserDetectResponse {
                        items: value.into_iter().map(Into::into).collect(),
                    })
                })
            }
            Call::BrowserSessionInfo(p) => {
                let p: IdParams = p.try_into()?;
                let result: Result<SessionArchiveInfo> = {
                    let ip: IdParams = p;
                    Ok(self.read(|v| self.browsers().session_info(v, &ip.id))?)
                };
                result.map(|value| Reply::BrowserSessionInfo(value.into()))
            }
            Call::SyncStatus(_) => {
                let result: Result<SyncStatus> = { Ok(self.read(|v| v.sync_status())?) };
                result.map(|value| Reply::SyncStatus(value.into()))
            }
            Call::SyncRun(p) => {
                let p: SyncParams = p.try_into()?;
                let result: Result<SyncResult> = {
                    let sp: SyncParams = p;
                    let r = self.write(|v| v.sync(&sp))?;
                    self.emit(AgentEventKind::SyncChanged, None, None, None);
                    if r.pulled > 0 {
                        self.emit(AgentEventKind::ProfilesChanged, None, None, None);
                        self.emit(AgentEventKind::EntriesChanged, None, None, None);
                    }
                    Ok(r)
                };
                result.map(|value| Reply::SyncRun(value.into()))
            }
            Call::SyncConflict(p) => {
                let p: IdParams = p.try_into()?;
                let result: Result<ConflictDetail> = {
                    let ip: IdParams = p;
                    Ok(self.read(|v| v.conflict_detail(&ip.id))?)
                };
                result.map(|value| Reply::SyncConflict(value.into()))
            }
            Call::SyncResolve(p) => {
                let p: ResolveParams = p.try_into()?;
                let result: Result<SyncStatus> = {
                    let rp: ResolveParams = p;
                    let r = self.write(|v| v.resolve(&rp))?;
                    self.emit(AgentEventKind::SyncChanged, None, None, None);
                    self.emit(
                        AgentEventKind::EntriesChanged,
                        None,
                        Some(rp.entry_id),
                        None,
                    );
                    Ok(r)
                };
                result.map(|value| Reply::SyncResolve(value.into()))
            }
            Call::SyncSetRemote(p) => {
                let p: SetRemoteParams = p.try_into()?;
                let result: Result<SyncStatus> = {
                    let sp: SetRemoteParams = p;
                    let r = self.write(|v| v.set_remote(&sp.url))?;
                    self.emit(AgentEventKind::SyncChanged, None, None, None);
                    Ok(r)
                };
                result.map(|value| Reply::SyncSetRemote(value.into()))
            }
            Call::RecipientsList(_) => {
                let result: Result<Vec<RecipientInfo>> =
                    { Ok(self.read(|v| Ok(v.list_recipients()))?) };
                result.map(|value| {
                    Reply::RecipientsList(v1::RecipientsListResponse {
                        items: value.into_iter().map(Into::into).collect(),
                    })
                })
            }
            Call::RecipientsAdd(p) => {
                let p: AddRecipientParams = p.try_into()?;
                let result: Result<Vec<RecipientInfo>> = {
                    let ap: AddRecipientParams = p;
                    let r = self.write(|v| v.add_recipient(&ap))?;
                    self.emit(AgentEventKind::VaultChanged, None, None, None);
                    Ok(r)
                };
                result.map(|value| {
                    Reply::RecipientsAdd(v1::RecipientsAddResponse {
                        items: value.into_iter().map(Into::into).collect(),
                    })
                })
            }
            Call::RecipientsRemove(p) => {
                let p: RemoveRecipientParams = p.try_into()?;
                let result: Result<Vec<RecipientInfo>> = {
                    let rp: RemoveRecipientParams = p;
                    let r = self.write(|v| v.remove_recipient(&rp))?;
                    self.emit(AgentEventKind::VaultChanged, None, None, None);
                    Ok(r)
                };
                result.map(|value| {
                    Reply::RecipientsRemove(v1::RecipientsRemoveResponse {
                        items: value.into_iter().map(Into::into).collect(),
                    })
                })
            }
            Call::RecipientsRotate(_) => {
                let result: Result<Vec<RecipientInfo>> = {
                    let r = self.write(|v| v.rotate())?;
                    self.emit(AgentEventKind::VaultChanged, None, None, None);
                    Ok(r)
                };
                result.map(|value| {
                    Reply::RecipientsRotate(v1::RecipientsRotateResponse {
                        items: value.into_iter().map(Into::into).collect(),
                    })
                })
            }
            Call::ImportPreview(p) => {
                let p: ImportParams = p.try_into()?;
                let result: Result<ImportPreview> = {
                    let ip: ImportParams = p;
                    Ok(self.read(|v| crate::core::import::preview(v, &ip))?)
                };
                result.map(|value| Reply::ImportPreview(value.into()))
            }
            Call::ImportApply(p) => {
                let p: ImportParams = p.try_into()?;
                let result: Result<ImportApplyResult> = {
                    let ip: ImportParams = p;
                    let r = self.write(|v| crate::core::import::apply(v, &ip))?;
                    self.emit(AgentEventKind::ProfilesChanged, None, None, None);
                    self.emit(AgentEventKind::EntriesChanged, None, None, None);
                    Ok(r)
                };
                result.map(|value| Reply::ImportApply(value.into()))
            }
            Call::ExportRun(p) => {
                let p: ExportParams = p.try_into()?;
                let result: Result<ExportResult> = {
                    let ep: ExportParams = p;
                    Ok(self.read(|v| crate::core::export::run(v, &ep))?)
                };
                result.map(|value| Reply::ExportRun(value.into()))
            }
            Call::DoctorRun(p) => {
                let p: DoctorParams = p.try_into()?;
                let result: Result<DoctorReport> = { self.doctor(p) };
                result.map(|value| Reply::DoctorRun(value.into()))
            }
            Call::SettingsGet(_) => {
                let result: Result<Settings> = { Ok(self.settings_get()) };
                result.map(|value| Reply::SettingsGet(value.into()))
            }
            Call::SettingsUpdate(p) => {
                let p: Settings = p.try_into()?;
                let result: Result<Settings> = { self.settings_update(p) };
                result.map(|value| Reply::SettingsUpdate(value.into()))
            }
            Call::VaultLocate(p) => {
                let p: OpenParams = p.try_into()?;
                let result: Result<TextResult> = { Ok(local::vault_locate(self, p)?) };
                result.map(|value| Reply::VaultLocate(value.into()))
            }
            Call::DoctorScan(p) => {
                let p: OpenParams = p.try_into()?;
                let result: Result<DoctorReport> = { Ok(local::doctor_scan(p)?) };
                result.map(|value| Reply::DoctorScan(value.into()))
            }
            Call::HooksInstall(p) => {
                let p: HookParams = p.try_into()?;
                let result: Result<HookResult> = { Ok(local::hooks_install(p)?) };
                result.map(|value| Reply::HooksInstall(value.into()))
            }
            Call::HooksUninstall(p) => {
                let p: HookParams = p.try_into()?;
                let result: Result<HookResult> = { Ok(local::hooks_uninstall(p)?) };
                result.map(|value| Reply::HooksUninstall(value.into()))
            }
            Call::TotpParse(p) => {
                let p: EnvParseParams = p.try_into()?;
                let result: Result<TotpConfig> = { Ok(local::totp_parse(p)?) };
                result.map(|value| Reply::TotpParse(value.into()))
            }
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

    fn settings_update(&self, new: Settings) -> Result<Settings> {
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
        Ok(self.settings_get())
    }

    // ----- clipboard --------------------------------------------------------

    fn clipboard_copy(&self, cp: ClipboardCopyParams) -> Result<ClipboardCopyResult> {
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
        Ok(ClipboardCopyResult {
            clear_after_secs: secs,
        })
    }
}
