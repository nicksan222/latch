import type {
  EntrySummary,
  Settings,
  SyncStatus,
  Theme,
  VaultStatus
} from '../_generated/contracts';
import { RpcError } from '../lib/api/errors';
import type { Transport } from '../lib/api/transport';
import { demoEntries, demoProfiles, demoValue } from './data';

/** Public fixture responses only. No agent, filesystem, encryption, or network access. */
export function createDemoTransport(): Transport & { setTheme(theme: Theme): void } {
  let locked = false;
  let theme: Theme = 'dark';
  const entries = demoProfiles.flatMap((profile) => demoEntries(profile.id));
  const settings = (): Settings => ({
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
  const sync: SyncStatus = {
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
  };
  const status = (): VaultStatus => ({
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
    locksInSecs: null,
    trust: null,
    integrityProblems: [],
    rotationPending: false,
    memoryLocked: null
  });
  return {
    kind: 'stub',
    setTheme(value) {
      theme = value;
    },
    subscribe: () => () => {},
    async call(method, params) {
      const args = (params ?? {}) as {
        id?: string;
        field?: string;
        profileId?: string;
        query?: string;
        type?: string;
        reveal?: boolean;
      };
      if (method === 'vault.status') return status();
      if (method === 'vault.list') return [];
      if (method === 'settings.get') return settings();
      if (method === 'vault.lock') {
        locked = true;
        return null;
      }
      if (method === 'vault.unlock') {
        locked = false;
        return status();
      }
      if (locked) throw new RpcError('locked', 'Unlock the demo or use Reset demo.');
      if (method === 'profiles.list')
        return demoProfiles.map((profile) => ({
          ...profile,
          pinned: false,
          browserRunning: false,
          entryCount: entries.filter((entry) => entry.profileId === profile.id).length
        }));
      if (method === 'entries.list') {
        const query = args.query?.toLowerCase() ?? '';
        return entries
          .filter(
            (entry) =>
              (!args.profileId || entry.profileId === args.profileId) &&
              (!args.type || entry.payload.type === args.type) &&
              `${entry.name} ${entry.tags.join(' ')} ${entry.payload.type}`
                .toLowerCase()
                .includes(query)
          )
          .map((entry): EntrySummary => ({
            id: entry.id,
            profileId: entry.profileId,
            name: entry.name,
            type: entry.payload.type,
            tags: entry.tags,
            favorite: entry.favorite,
            updatedAt: entry.updatedAt,
            hasTotp: false,
            subtitle:
              entry.payload.type === 'login'
                ? entry.payload.data.username
                : entry.payload.type === 'database'
                  ? entry.payload.data.host
                  : entry.payload.type === 'env'
                    ? `${entry.payload.data.vars.length} vars`
                    : null
          }));
      }
      if (method === 'entries.get') {
        const entry = entries.find((entry) => entry.id === args.id);
        if (!entry) throw new RpcError('not_found', 'Demo entry not found');
        // Editing/writing needs the real agent; never pretend a demo mutation was saved.
        if (args.reveal)
          throw new RpcError(
            'invalid_params',
            'Editing is available in the desktop app. This preview uses read-only fictional data.'
          );
        return structuredClone(entry);
      }
      if (method === 'entries.reveal' || method === 'clipboard.copy') {
        const value = demoValue(args.id ?? '', args.field ?? '');
        if (method === 'entries.reveal') return { value };
        await navigator.clipboard.writeText(value);
        return { clearAfterSecs: 0 };
      }
      if (method === 'entries.history')
        return [
          {
            commit: '8e4b2c7f13a098d6',
            at: '2026-10-25T12:00:00Z',
            author: 'Alex Morgan',
            message: 'Update encrypted entry',
            changedFields: ['password'],
            deleted: false
          }
        ];
      if (method === 'sync.status') return sync;
      if (['entries.trash', 'recipients.list', 'browser.list', 'totp.pinned'].includes(method))
        return [];
      throw new RpcError(
        'invalid_params',
        'This action is available in the desktop app. The public demo uses read-only fictional data.'
      );
    }
  };
}
