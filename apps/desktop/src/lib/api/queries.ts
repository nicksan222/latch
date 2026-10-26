/**
 * TanStack Query hooks per domain. Only redacted data is cached: revealed
 * values (`entries.reveal`, `entries.get { reveal: true }`, `sync.conflict`)
 * are fetched imperatively by the component that asked for them.
 *
 * Mutations carry their inputs as `variables` (entry passwords, import
 * payloads, merged conflict entries…). None of that may outlive the call:
 * every RPC mutation uses `gcTime: 0`, is removed from the MutationCache as
 * soon as it settles and its observer is reset, and lock / navigation clear
 * the whole MutationCache. Completions that land after a lock are ignored.
 */
import {
  MutationCache,
  QueryClient,
  useMutation,
  useQuery,
  useQueryClient,
  type QueryKey
} from '@tanstack/react-query';
import { useEffect, useRef } from 'react';
import type { AgentEvent, EntryType, ListEntriesParams } from '../../_generated/contracts';
import { call, onAgentEvent } from './client';
import { isRpcError } from './errors';
import type { Method, ParamsArg, ParamsOf, ResultOf } from './methods';

export const qk = {
  status: ['vault', 'status'] as const,
  vaults: ['vault', 'list'] as const,
  settings: ['settings'] as const,
  profiles: ['profiles'] as const,
  actionList: (id: string) => ['profiles', id, 'actions'] as const,
  profile: (id: string) => ['profiles', id] as const,
  entries: ['entries'] as const,
  entryList: (params: ListEntriesParams) => ['entries', 'list', params] as const,
  entry: (id: string) => ['entries', 'detail', id] as const,
  history: (id: string) => ['entries', 'history', id] as const,
  trash: ['entries', 'trash'] as const,
  sync: ['sync'] as const,
  recipients: ['recipients'] as const,
  browser: ['browser'] as const,
  browserList: ['browser', 'list'] as const,
  browserDetect: ['browser', 'detect'] as const,
  sessionInfo: (id: string) => ['browser', 'session', id] as const,
  totpPinned: ['totp', 'pinned'] as const
};

/** Marks mutations whose variables may hold plaintext (all RPC mutations). */
export const SCRUB_META = { scrub: true } as const;

/**
 * Bumped on every lock. A mutation started in an older epoch must not run
 * its success/error side effects (invalidations, navigation, toasts).
 */
let lockEpoch = 0;

export function createQueryClient() {
  const mutationCache: MutationCache = new MutationCache({
    onSettled: (_data, _error, _variables, _onMutateResult, mutation) => {
      // Drop settled mutations (and their variables) right away.
      if (mutation.meta?.scrub) mutationCache.remove(mutation);
    }
  });
  return new QueryClient({
    mutationCache,
    defaultOptions: {
      queries: {
        staleTime: 10_000,
        refetchOnWindowFocus: false,
        retry: (count, error) => !isRpcError(error) && count < 1
      },
      mutations: { retry: false, gcTime: 0 }
    }
  });
}

/** Drops every mutation (with its variables) from the cache. */
export function clearMutations(queryClient: QueryClient) {
  queryClient.getMutationCache().clear();
}

/**
 * Lock: forget all cached entry metadata and every mutation's variables,
 * and invalidate in-flight mutations so late completions cannot repopulate
 * anything.
 */
export function scrubOnLock(queryClient: QueryClient) {
  lockEpoch += 1;
  forgetVaultData(queryClient);
}

/** Vault switched/changed under us: drop cached metadata and mutation inputs. */
function forgetVaultData(queryClient: QueryClient) {
  clearMutations(queryClient);
  queryClient.removeQueries({
    predicate: (query) => query.queryKey[0] !== 'vault' && query.queryKey[0] !== 'settings'
  });
}

/** Thin typed `useQuery` around `call`. */
function useRpc<K extends Method>(
  key: QueryKey,
  method: K,
  args: ParamsArg<K>,
  options: { enabled?: boolean; refetchInterval?: number | false } = {}
) {
  return useQuery<ResultOf<K>>({
    queryKey: key,
    queryFn: () => call(method, ...args),
    ...options
  });
}

export const useVaultStatus = (refetchInterval: number | false = false) =>
  useRpc(qk.status, 'vault.status', [], { refetchInterval });
export const useKnownVaults = () => useRpc(qk.vaults, 'vault.list', []);
export const useSettings = (enabled = true) => useRpc(qk.settings, 'settings.get', [], { enabled });
export const useActions = (id: string, enabled = true) =>
  useRpc(qk.actionList(id), 'actions.list', [{ id }], { enabled });
export const useProfiles = () => useRpc(qk.profiles, 'profiles.list', []);
export const useProfile = (id: string | undefined) =>
  useRpc(qk.profile(id ?? ''), 'profiles.get', [{ id: id ?? '' }], { enabled: Boolean(id) });

export function useEntries(params: {
  profileId?: string | null;
  query?: string;
  type?: EntryType | null;
}) {
  const p: ListEntriesParams = {
    profileId: params.profileId ?? null,
    query: params.query?.trim() ? params.query.trim() : null,
    type: params.type ?? null
  };
  return useRpc(qk.entryList(p), 'entries.list', [p]);
}

/** Redacted entry (no secrets). */
export const useEntry = (id: string | undefined) =>
  useRpc(qk.entry(id ?? ''), 'entries.get', [{ id: id ?? '', reveal: false }], {
    enabled: Boolean(id)
  });
export const useEntryHistory = (id: string | undefined) =>
  useRpc(qk.history(id ?? ''), 'entries.history', [{ id: id ?? '' }], { enabled: Boolean(id) });
export const useTrash = () => useRpc(qk.trash, 'entries.trash', [{ profileId: null }]);
export const useSyncStatus = () => useRpc(qk.sync, 'sync.status', []);
export const useRecipients = () => useRpc(qk.recipients, 'recipients.list', []);
export const useBrowserSessions = () => useRpc(qk.browserList, 'browser.list', []);
export const useSessionInfo = (profileId: string | undefined) =>
  useRpc(qk.sessionInfo(profileId ?? ''), 'browser.sessionInfo', [{ id: profileId ?? '' }], {
    enabled: Boolean(profileId)
  });

/**
 * Typed mutation; invalidates the given keys on success. Inputs never
 * linger: `gcTime: 0`, removed from the cache on settle (see
 * `createQueryClient`), and the observer is reset once the result has been
 * delivered, so `variables` is not retained by a mounted component either.
 */
export function useRpcMutation<K extends Method, Variables extends ParamsOf<K> = ParamsOf<K>>(
  method: K,
  options: {
    invalidate?: readonly QueryKey[];
    /** Strip presentation-only fields before sending mutation inputs to the agent. */
    toParams?: (variables: Variables) => ParamsOf<K>;
    onSuccess?: (result: ResultOf<K>, params: Variables) => void;
    onError?: (error: unknown, params: Variables) => void;
  } = {}
) {
  const queryClient = useQueryClient();
  // Earlier concurrent mutations no longer receive TanStack observer option updates.
  // Keep their callbacks current too, including callbacks that capture UI state.
  const optionsRef = useRef(options);
  useEffect(() => {
    optionsRef.current = options;
  }, [options]);
  const mutation = useMutation<ResultOf<K>, unknown, Variables, { epoch: number }>({
    mutationFn: (variables) => {
      const params = options.toParams ? options.toParams(variables) : variables;
      return call(method, ...([params] as unknown as ParamsArg<K>));
    },
    gcTime: 0,
    meta: SCRUB_META,
    onMutate: () => ({ epoch: lockEpoch }),
    onSuccess: async (result, params, ctx) => {
      if (ctx?.epoch !== lockEpoch) return; // locked meanwhile: no side effects
      const { invalidate, onSuccess } = optionsRef.current;
      await Promise.all(
        (invalidate ?? []).map((queryKey) => queryClient.invalidateQueries({ queryKey }))
      );
      if (ctx?.epoch !== lockEpoch) return;
      onSuccess?.(result, params);
    },
    onError: (error, params, ctx) => {
      if (ctx !== undefined && ctx.epoch !== lockEpoch) return;
      optionsRef.current.onError?.(error, params);
    }
  });
  const { isSuccess, isError, reset } = mutation;
  useEffect(() => {
    if (isSuccess || isError) reset(); // forget variables/data once delivered
  }, [isSuccess, isError, reset]);
  return mutation;
}

/** Maps an agent event to the query keys it invalidates. */
export function keysForEvent(event: AgentEvent): QueryKey[] {
  switch (event.kind) {
    case 'locked':
    case 'unlocked':
    case 'vault_changed':
      return [[]];
    case 'profiles_changed':
      return [qk.profiles, qk.entries, qk.totpPinned];
    case 'entries_changed':
      return [qk.entries, qk.profiles, qk.totpPinned];
    case 'browser_changed':
      return [qk.browser, qk.profiles];
    case 'sync_changed':
      return [qk.sync, qk.status];
    case 'lock_warning':
      return [qk.status];
  }
}

/**
 * Wires `latch://event` to the query cache. On `locked`, every cached query
 * and every mutation (with its variables) is dropped before status refetches.
 */
export function useAgentEvents(onEvent?: (event: AgentEvent) => void) {
  const queryClient = useQueryClient();
  useEffect(
    () =>
      onAgentEvent((event) => {
        if (event.kind === 'locked' || event.kind === 'vault_changed') {
          // Only a lock invalidates in-flight mutations (a vault change can be
          // the result of one, e.g. recipients.add re-encrypting).
          if (event.kind === 'locked') scrubOnLock(queryClient);
          else {
            clearMutations(queryClient);
            // Reset retains active observers and refetches them. Removing a
            // pending query here leaves a mounted view waiting forever.
            void queryClient.resetQueries({
              predicate: (query) =>
                query.queryKey[0] !== 'vault' && query.queryKey[0] !== 'settings'
            });
          }
          void queryClient.invalidateQueries({ queryKey: ['vault'] });
        } else {
          for (const queryKey of keysForEvent(event))
            void queryClient.invalidateQueries({ queryKey });
        }
        onEvent?.(event);
      }),
    [queryClient, onEvent]
  );
}
