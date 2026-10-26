import { create, fromBinary, toBinary } from '@bufbuild/protobuf';
import {
  EntryType,
  ErrorCode,
  LatchAgent,
  SettingsSchema,
  SyncStatusSchema,
  Theme,
  VaultStatusSchema
} from '@latch/rpc/v1';
import { RpcError } from '../lib/api/errors';
import type { Transport } from '../lib/api/transport';
import { demoEntries, demoProfiles, demoValue } from './data';

/** Public fixture responses only. No agent, filesystem, encryption, or network access. */
export function createDemoTransport(): Transport & { setTheme(theme: Theme): void } {
  let locked = false;
  let theme = Theme.DARK;
  const entries = demoProfiles.flatMap((profile) => demoEntries(profile.id));
  const settings = () =>
    create(SettingsSchema, {
      autoLockSecs: 900,
      clipboardClearSecs: 30,
      revealHideSecs: 20,
      lockOnScreenLock: true,
      trayEnabled: true,
      autostart: false,
      theme,
      globalShortcut: 'CommandOrControl+Shift+L',
      keepBrowsersOpenOnLock: false,
      namesInCommits: false,
      autoPush: false,
      sessionsCommit: false,
      gitName: 'Alex Morgan',
      gitEmail: 'alex@example.com'
    });
  const sync = create(SyncStatusSchema, {
    hasRemote: true,
    remoteUrl: 'git@github.com:alex-morgan/vault.git',
    branch: 'main',
    ahead: 0,
    behind: 0,
    dirty: false,
    lastSyncAt: '2026-10-25T12:00:00Z',
    conflicts: [],
    log: [
      {
        commit: '8e4b2c7f13a098d6',
        at: '2026-10-25T12:00:00Z',
        author: 'Alex Morgan',
        message: 'Update encrypted entry'
      }
    ]
  });
  const status = () =>
    create(VaultStatusSchema, {
      initialized: true,
      locked,
      vaultId: 'demo-vault-7f28ac19',
      path: '/home/alex/vault',
      recipients: [],
      dirty: false,
      ahead: 0,
      behind: 0,
      hasRemote: true,
      lastSyncAt: sync.lastSyncAt,
      locksInSecs: undefined,
      trust: undefined,
      integrityProblems: [],
      rotationPending: false,
      memoryLocked: undefined
    });
  return {
    kind: 'stub',
    setTheme(value) {
      theme = value;
    },
    subscribe: () => () => {},
    async request(method, payload) {
      const respond = (value: Parameters<typeof create<typeof method.output>>[1]) =>
        toBinary(method.output, create(method.output, value));
      // Decode every request against its official input descriptor.
      fromBinary(method.input, payload);
      if (method === LatchAgent.method.vaultStatus) return respond(status());
      if (method === LatchAgent.method.vaultList) return respond({ items: [] });
      if (method === LatchAgent.method.settingsGet) return respond(settings());
      if (method === LatchAgent.method.vaultLock) {
        locked = true;
        return respond(status());
      }
      if (method === LatchAgent.method.vaultUnlock) {
        locked = false;
        return respond(status());
      }
      if (locked) throw new RpcError(ErrorCode.LOCKED, 'Unlock the demo or use Reset demo.');
      if (method === LatchAgent.method.profilesList)
        return respond({
          items: demoProfiles.map((profile) => ({
            ...profile,
            pinned: false,
            browserRunning: false,
            entryCount: entries.filter((entry) => entry.profileId === profile.id).length
          }))
        });
      if (method === LatchAgent.method.entriesList) {
        const args = fromBinary(LatchAgent.method.entriesList.input, payload);
        const types = {
          login: EntryType.LOGIN,
          database: EntryType.DATABASE,
          apiKey: EntryType.API_KEY,
          env: EntryType.ENV,
          secret: EntryType.SECRET,
          note: EntryType.NOTE
        };
        const entryType = (entry: (typeof entries)[number]) =>
          types[entry.payload!.data.case as keyof typeof types] ?? EntryType.UNSPECIFIED;
        const query = args.query?.toLowerCase() ?? '';
        return respond({
          items: entries
            .filter(
              (entry) =>
                (!args.profileId || entry.profileId === args.profileId) &&
                (!args.type || entryType(entry) === args.type) &&
                `${entry.name} ${entry.tags.join(' ')} ${entry.payload!.data.case}`
                  .toLowerCase()
                  .includes(query)
            )
            .map((entry) => ({
              id: entry.id,
              profileId: entry.profileId,
              name: entry.name,
              type: entryType(entry),
              tags: entry.tags,
              favorite: entry.favorite,
              updatedAt: entry.updatedAt,
              hasTotp: false,
              subtitle:
                entry.payload?.data.case === 'login'
                  ? entry.payload.data.value.username
                  : entry.payload?.data.case === 'database'
                    ? entry.payload.data.value.host
                    : entry.payload?.data.case === 'env'
                      ? `${entry.payload.data.value.vars.length} vars`
                      : undefined
            }))
        });
      }
      if (method === LatchAgent.method.entriesGet) {
        const args = fromBinary(LatchAgent.method.entriesGet.input, payload);
        const entry = entries.find((entry) => entry.id === args.id);
        if (!entry) throw new RpcError(ErrorCode.NOT_FOUND, 'Demo entry not found');
        // Editing/writing needs the real agent; never pretend a demo mutation was saved.
        if (args.reveal)
          throw new RpcError(
            ErrorCode.INVALID_PARAMS,
            'Editing is available in the desktop app. This preview uses read-only fictional data.'
          );
        return respond(entry);
      }
      if (
        method === LatchAgent.method.entriesReveal ||
        method === LatchAgent.method.clipboardCopy
      ) {
        const args =
          method === LatchAgent.method.entriesReveal
            ? fromBinary(LatchAgent.method.entriesReveal.input, payload)
            : fromBinary(LatchAgent.method.clipboardCopy.input, payload);
        const value = demoValue(args.id ?? '', args.field ?? '');
        if (method === LatchAgent.method.entriesReveal) return respond({ value });
        await navigator.clipboard.writeText(value);
        return respond({ clearAfterSecs: 0 });
      }
      if (method === LatchAgent.method.entriesHistory)
        return respond({
          items: [
            {
              commit: '8e4b2c7f13a098d6',
              at: '2026-10-25T12:00:00Z',
              author: 'Alex Morgan',
              message: 'Update encrypted entry',
              changedFields: ['password'],
              deleted: false
            }
          ]
        });
      if (method === LatchAgent.method.syncStatus) return respond(sync);
      if (
        [
          LatchAgent.method.entriesTrash,
          LatchAgent.method.recipientsList,
          LatchAgent.method.browserList,
          LatchAgent.method.totpPinned
        ].some((item) => item === method)
      )
        return respond({ items: [] });
      throw new RpcError(
        ErrorCode.INVALID_PARAMS,
        'This action is available in the desktop app. The public demo uses read-only fictional data.'
      );
    }
  };
}
