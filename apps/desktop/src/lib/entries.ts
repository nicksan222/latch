/** Presentation projections of the generated Rust catalog and thin domain RPCs. */
import type { Entry, EntryInput, EntryPayload, EntryType } from '../_generated/contracts';
import domainCatalog from '../_generated/contracts/domain-catalog.json';
import { domain } from './domain';

export const ENTRY_TYPES = domainCatalog.entryTypes.map((entry) => entry.type) as EntryType[];
export const ENTRY_TYPE_LABEL = Object.fromEntries(
  domainCatalog.entryTypes.map((entry) => [entry.type, entry.label])
) as Record<EntryType, string>;
export const PRIMARY_COPY_FIELD = Object.fromEntries(
  domainCatalog.entryTypes.map((entry) => [entry.type, entry.primaryCopyField])
) as Record<EntryType, string | null>;
export type PayloadOf<T extends EntryType> = Extract<EntryPayload, { type: T }>['data'];

export function emptyPayload(type: EntryType): EntryPayload {
  const metadata = domainCatalog.entryTypes.find((entry) => entry.type === type);
  if (!metadata) throw new Error('Unknown entry type');
  return structuredClone(metadata.defaultPayload) as EntryPayload;
}
export function emptyEntryInput(profileId: string, type: EntryType): EntryInput {
  return { profileId, name: '', tags: [], notes: '', favorite: false, payload: emptyPayload(type) };
}
export const redactPayload = (payload: EntryPayload) => domain('redact_payload', payload);
export const mergeKeepOnNone = (stored: EntryPayload, input: EntryPayload) =>
  domain('merge_keep_on_none', { stored, input });
export const mergeEntries = (mine: Entry, theirs: Entry, fromTheirs: ReadonlySet<string>) =>
  domain('merge_entries', {
    mine: entryToInput(mine),
    theirs: entryToInput(theirs),
    fromTheirs: [...fromTheirs]
  });

export function entryToInput(entry: Entry | EntryInput): EntryInput {
  return {
    profileId: entry.profileId,
    name: entry.name,
    tags: [...entry.tags],
    notes: entry.notes,
    favorite: entry.favorite,
    payload: structuredClone(entry.payload)
  };
}

export function formatBytes(size: number): string {
  if (size < 1024) return `${size} B`;
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`;
  return `${(size / 1024 / 1024).toFixed(1)} MB`;
}
