// Generated from the Protobuf descriptor via procedures.json. Do not edit.
import { invoke as officialInvoke } from '@tauri-apps/api/core';
import type { ActionConfigureParams, ActionIdParams, ActionListResult, ActionSaveParams, AddRecipientParams, BrowserLaunchParams, BrowserSession, ChangePassphraseParams, ClipboardCopyParams, ClipboardCopyResult, CloneParams, ConflictDetail, DetectedBrowser, DoctorParams, DoctorReport, DomainRequest, DomainResponse, Entry, EntryInput, EntrySummary, EntryUpdateParams, EnvFormatParams, EnvParseParams, EnvResolveParams, EnvResolveResult, EnvVar, ExportParams, ExportResult, ForgetTrustParams, GetEntryParams, HelloParams, HelloResult, HistoryItem, HookParams, HookResult, IdParams, ImportApplyResult, ImportParams, ImportPreview, InitParams, InitResult, KdfBenchmark, KdfBenchmarkParams, KnownVault, LaunchAction, ListEntriesParams, MoveEntryParams, OpenParams, PinnedTotp, Profile, ProfileInput, ProfileSummary, ProfileUpdateParams, RecipientInfo, RemoveRecipientParams, ResolveParams, RestoreParams, RevealParams, RevealResult, SessionArchiveInfo, SetRemoteParams, Settings, SyncParams, SyncResult, SyncStatus, TextResult, TotpCode, TotpConfig, TrashItem, TrashParams, TrustStatus, UnlockParams, VaultStatus } from '../_generated/contracts';

export function createDesktopSdk(invoke: typeof officialInvoke = officialInvoke) {
  const procedures = {
    domain: {
      call: (params: DomainRequest): Promise<DomainResponse> => invoke<DomainResponse>('rpc', { method: 'domain.call', params: params }),
    },
    hello: (params: HelloParams): Promise<HelloResult> => invoke<HelloResult>('rpc', { method: 'hello', params: params }),
    vault: {
      status: (): Promise<VaultStatus> => invoke<VaultStatus>('rpc', { method: 'vault.status', params: null }),
      init: (params: InitParams): Promise<InitResult> => invoke<InitResult>('rpc', { method: 'vault.init', params: params }),
      clone: (params: CloneParams): Promise<VaultStatus> => invoke<VaultStatus>('rpc', { method: 'vault.clone', params: params }),
      open: (params: OpenParams): Promise<VaultStatus> => invoke<VaultStatus>('rpc', { method: 'vault.open', params: params }),
      list: (): Promise<Array<KnownVault>> => invoke<Array<KnownVault>>('rpc', { method: 'vault.list', params: null }),
      unlock: (params: UnlockParams): Promise<VaultStatus> => invoke<VaultStatus>('rpc', { method: 'vault.unlock', params: params }),
      lock: (): Promise<VaultStatus> => invoke<VaultStatus>('rpc', { method: 'vault.lock', params: null }),
      benchmarkKdf: (params: KdfBenchmarkParams): Promise<KdfBenchmark> => invoke<KdfBenchmark>('rpc', { method: 'vault.benchmarkKdf', params: params }),
      changePassphrase: (params: ChangePassphraseParams): Promise<null> => invoke<null>('rpc', { method: 'vault.changePassphrase', params: params }),
      trust: (): Promise<TrustStatus> => invoke<TrustStatus>('rpc', { method: 'vault.trust', params: {} }),
      forgetTrust: (params: ForgetTrustParams): Promise<TrustStatus> => invoke<TrustStatus>('rpc', { method: 'vault.forgetTrust', params: params }),
      locate: (params: OpenParams): Promise<TextResult> => invoke<TextResult>('rpc', { method: 'vault.locate', params: params }),
    },
    profiles: {
      list: (): Promise<Array<ProfileSummary>> => invoke<Array<ProfileSummary>>('rpc', { method: 'profiles.list', params: null }),
      get: (params: IdParams): Promise<Profile> => invoke<Profile>('rpc', { method: 'profiles.get', params: params }),
      create: (params: ProfileInput): Promise<Profile> => invoke<Profile>('rpc', { method: 'profiles.create', params: params }),
      update: (params: ProfileUpdateParams): Promise<Profile> => invoke<Profile>('rpc', { method: 'profiles.update', params: params }),
      delete: (params: IdParams): Promise<null> => invoke<null>('rpc', { method: 'profiles.delete', params: params }),
    },
    entries: {
      list: (params: ListEntriesParams): Promise<Array<EntrySummary>> => invoke<Array<EntrySummary>>('rpc', { method: 'entries.list', params: params }),
      get: (params: GetEntryParams): Promise<Entry> => invoke<Entry>('rpc', { method: 'entries.get', params: params }),
      reveal: (params: RevealParams): Promise<RevealResult> => invoke<RevealResult>('rpc', { method: 'entries.reveal', params: params }),
      create: (params: EntryInput): Promise<Entry> => invoke<Entry>('rpc', { method: 'entries.create', params: params }),
      update: (params: EntryUpdateParams): Promise<Entry> => invoke<Entry>('rpc', { method: 'entries.update', params: params }),
      delete: (params: IdParams): Promise<null> => invoke<null>('rpc', { method: 'entries.delete', params: params }),
      move: (params: MoveEntryParams): Promise<Entry> => invoke<Entry>('rpc', { method: 'entries.move', params: params }),
      history: (params: IdParams): Promise<Array<HistoryItem>> => invoke<Array<HistoryItem>>('rpc', { method: 'entries.history', params: params }),
      restore: (params: RestoreParams): Promise<Entry> => invoke<Entry>('rpc', { method: 'entries.restore', params: params }),
      trash: (params: TrashParams): Promise<Array<TrashItem>> => invoke<Array<TrashItem>>('rpc', { method: 'entries.trash', params: params }),
    },
    actions: {
      list: (params: IdParams): Promise<ActionListResult> => invoke<ActionListResult>('rpc', { method: 'actions.list', params: params }),
      save: (params: ActionSaveParams): Promise<LaunchAction> => invoke<LaunchAction>('rpc', { method: 'actions.save', params: params }),
      delete: (params: ActionIdParams): Promise<null> => invoke<null>('rpc', { method: 'actions.delete', params: params }),
      configure: (params: ActionConfigureParams): Promise<null> => invoke<null>('rpc', { method: 'actions.configure', params: params }),
      run: (params: ActionIdParams): Promise<null> => invoke<null>('rpc', { method: 'actions.run', params: params }),
    },
    env: {
      resolve: (params: EnvResolveParams): Promise<EnvResolveResult> => invoke<EnvResolveResult>('rpc', { method: 'env.resolve', params: params }),
      format: (params: EnvFormatParams): Promise<TextResult> => invoke<TextResult>('rpc', { method: 'env.format', params: params }),
      parse: (params: EnvParseParams): Promise<Array<EnvVar>> => invoke<Array<EnvVar>>('rpc', { method: 'env.parse', params: params }),
      copy: (params: EnvFormatParams): Promise<ClipboardCopyResult> => invoke<ClipboardCopyResult>('rpc', { method: 'env.copy', params: params }),
    },
    totp: {
      get: (params: IdParams): Promise<TotpCode> => invoke<TotpCode>('rpc', { method: 'totp.get', params: params }),
      pinned: (): Promise<Array<PinnedTotp>> => invoke<Array<PinnedTotp>>('rpc', { method: 'totp.pinned', params: null }),
      parse: (params: EnvParseParams): Promise<TotpConfig> => invoke<TotpConfig>('rpc', { method: 'totp.parse', params: params }),
    },
    clipboard: {
      copy: (params: ClipboardCopyParams): Promise<ClipboardCopyResult> => invoke<ClipboardCopyResult>('rpc', { method: 'clipboard.copy', params: params }),
    },
    browser: {
      launch: (params: BrowserLaunchParams): Promise<BrowserSession> => invoke<BrowserSession>('rpc', { method: 'browser.launch', params: params }),
      list: (): Promise<Array<BrowserSession>> => invoke<Array<BrowserSession>>('rpc', { method: 'browser.list', params: null }),
      close: (params: IdParams): Promise<null> => invoke<null>('rpc', { method: 'browser.close', params: params }),
      reset: (params: IdParams): Promise<null> => invoke<null>('rpc', { method: 'browser.reset', params: params }),
      detect: (): Promise<Array<DetectedBrowser>> => invoke<Array<DetectedBrowser>>('rpc', { method: 'browser.detect', params: null }),
      sessionInfo: (params: IdParams): Promise<SessionArchiveInfo> => invoke<SessionArchiveInfo>('rpc', { method: 'browser.sessionInfo', params: params }),
    },
    sync: {
      status: (): Promise<SyncStatus> => invoke<SyncStatus>('rpc', { method: 'sync.status', params: null }),
      run: (params: SyncParams): Promise<SyncResult> => invoke<SyncResult>('rpc', { method: 'sync.run', params: params }),
      conflict: (params: IdParams): Promise<ConflictDetail> => invoke<ConflictDetail>('rpc', { method: 'sync.conflict', params: params }),
      resolve: (params: ResolveParams): Promise<SyncStatus> => invoke<SyncStatus>('rpc', { method: 'sync.resolve', params: params }),
      setRemote: (params: SetRemoteParams): Promise<SyncStatus> => invoke<SyncStatus>('rpc', { method: 'sync.setRemote', params: params }),
    },
    recipients: {
      list: (): Promise<Array<RecipientInfo>> => invoke<Array<RecipientInfo>>('rpc', { method: 'recipients.list', params: null }),
      add: (params: AddRecipientParams): Promise<Array<RecipientInfo>> => invoke<Array<RecipientInfo>>('rpc', { method: 'recipients.add', params: params }),
      remove: (params: RemoveRecipientParams): Promise<Array<RecipientInfo>> => invoke<Array<RecipientInfo>>('rpc', { method: 'recipients.remove', params: params }),
      rotate: (): Promise<Array<RecipientInfo>> => invoke<Array<RecipientInfo>>('rpc', { method: 'recipients.rotate', params: null }),
    },
    import: {
      preview: (params: ImportParams): Promise<ImportPreview> => invoke<ImportPreview>('rpc', { method: 'import.preview', params: params }),
      apply: (params: ImportParams): Promise<ImportApplyResult> => invoke<ImportApplyResult>('rpc', { method: 'import.apply', params: params }),
    },
    export: {
      run: (params: ExportParams): Promise<ExportResult> => invoke<ExportResult>('rpc', { method: 'export.run', params: params }),
    },
    doctor: {
      run: (params: DoctorParams): Promise<DoctorReport> => invoke<DoctorReport>('rpc', { method: 'doctor.run', params: params }),
      scan: (params: OpenParams): Promise<DoctorReport> => invoke<DoctorReport>('rpc', { method: 'doctor.scan', params: params }),
    },
    settings: {
      get: (): Promise<Settings> => invoke<Settings>('rpc', { method: 'settings.get', params: null }),
      update: (params: Settings): Promise<Settings> => invoke<Settings>('rpc', { method: 'settings.update', params: params }),
    },
    hooks: {
      install: (params: HookParams): Promise<HookResult> => invoke<HookResult>('rpc', { method: 'hooks.install', params: params }),
      uninstall: (params: HookParams): Promise<HookResult> => invoke<HookResult>('rpc', { method: 'hooks.uninstall', params: params }),
    },
  };
  return {
    ...procedures,
    async call(method: string, params: unknown): Promise<unknown> {
      switch (method) {
      case 'domain.call': return procedures.domain.call(params as DomainRequest);
      case 'hello': return procedures.hello(params as HelloParams);
      case 'vault.status': return procedures.vault.status();
      case 'vault.init': return procedures.vault.init(params as InitParams);
      case 'vault.clone': return procedures.vault.clone(params as CloneParams);
      case 'vault.open': return procedures.vault.open(params as OpenParams);
      case 'vault.list': return procedures.vault.list();
      case 'vault.unlock': return procedures.vault.unlock(params as UnlockParams);
      case 'vault.lock': return procedures.vault.lock();
      case 'vault.benchmarkKdf': return procedures.vault.benchmarkKdf(params as KdfBenchmarkParams);
      case 'vault.changePassphrase': return procedures.vault.changePassphrase(params as ChangePassphraseParams);
      case 'vault.trust': return procedures.vault.trust();
      case 'vault.forgetTrust': return procedures.vault.forgetTrust(params as ForgetTrustParams);
      case 'profiles.list': return procedures.profiles.list();
      case 'profiles.get': return procedures.profiles.get(params as IdParams);
      case 'profiles.create': return procedures.profiles.create(params as ProfileInput);
      case 'profiles.update': return procedures.profiles.update(params as ProfileUpdateParams);
      case 'profiles.delete': return procedures.profiles.delete(params as IdParams);
      case 'entries.list': return procedures.entries.list(params as ListEntriesParams);
      case 'entries.get': return procedures.entries.get(params as GetEntryParams);
      case 'entries.reveal': return procedures.entries.reveal(params as RevealParams);
      case 'entries.create': return procedures.entries.create(params as EntryInput);
      case 'entries.update': return procedures.entries.update(params as EntryUpdateParams);
      case 'entries.delete': return procedures.entries.delete(params as IdParams);
      case 'entries.move': return procedures.entries.move(params as MoveEntryParams);
      case 'entries.history': return procedures.entries.history(params as IdParams);
      case 'entries.restore': return procedures.entries.restore(params as RestoreParams);
      case 'entries.trash': return procedures.entries.trash(params as TrashParams);
      case 'actions.list': return procedures.actions.list(params as IdParams);
      case 'actions.save': return procedures.actions.save(params as ActionSaveParams);
      case 'actions.delete': return procedures.actions.delete(params as ActionIdParams);
      case 'actions.configure': return procedures.actions.configure(params as ActionConfigureParams);
      case 'actions.run': return procedures.actions.run(params as ActionIdParams);
      case 'env.resolve': return procedures.env.resolve(params as EnvResolveParams);
      case 'env.format': return procedures.env.format(params as EnvFormatParams);
      case 'env.parse': return procedures.env.parse(params as EnvParseParams);
      case 'env.copy': return procedures.env.copy(params as EnvFormatParams);
      case 'totp.get': return procedures.totp.get(params as IdParams);
      case 'totp.pinned': return procedures.totp.pinned();
      case 'clipboard.copy': return procedures.clipboard.copy(params as ClipboardCopyParams);
      case 'browser.launch': return procedures.browser.launch(params as BrowserLaunchParams);
      case 'browser.list': return procedures.browser.list();
      case 'browser.close': return procedures.browser.close(params as IdParams);
      case 'browser.reset': return procedures.browser.reset(params as IdParams);
      case 'browser.detect': return procedures.browser.detect();
      case 'browser.sessionInfo': return procedures.browser.sessionInfo(params as IdParams);
      case 'sync.status': return procedures.sync.status();
      case 'sync.run': return procedures.sync.run(params as SyncParams);
      case 'sync.conflict': return procedures.sync.conflict(params as IdParams);
      case 'sync.resolve': return procedures.sync.resolve(params as ResolveParams);
      case 'sync.setRemote': return procedures.sync.setRemote(params as SetRemoteParams);
      case 'recipients.list': return procedures.recipients.list();
      case 'recipients.add': return procedures.recipients.add(params as AddRecipientParams);
      case 'recipients.remove': return procedures.recipients.remove(params as RemoveRecipientParams);
      case 'recipients.rotate': return procedures.recipients.rotate();
      case 'import.preview': return procedures.import.preview(params as ImportParams);
      case 'import.apply': return procedures.import.apply(params as ImportParams);
      case 'export.run': return procedures.export.run(params as ExportParams);
      case 'doctor.run': return procedures.doctor.run(params as DoctorParams);
      case 'settings.get': return procedures.settings.get();
      case 'settings.update': return procedures.settings.update(params as Settings);
      case 'vault.locate': return procedures.vault.locate(params as OpenParams);
      case 'doctor.scan': return procedures.doctor.scan(params as OpenParams);
      case 'hooks.install': return procedures.hooks.install(params as HookParams);
      case 'hooks.uninstall': return procedures.hooks.uninstall(params as HookParams);
      case 'totp.parse': return procedures.totp.parse(params as EnvParseParams);
        default: throw new Error('Unknown RPC procedure');
      }
    },
  };
}
