/** Presentation projections of the Rust catalog and thin domain RPCs. */
import { create, fromJson, type JsonObject } from '@bufbuild/protobuf';
import {
  EntryInputSchema,
  EntryPayloadSchema,
  EntryType,
  type Entry,
  type EntryInput,
  type EntryPayload
} from '@latch/rpc/v1';
import domainCatalog from '../_generated/domain-catalog.json';
import { domain } from './domain';

/** The Rust catalog and URL paths retain their persisted snake_case names. */
const TYPE_BY_SLUG = {
  login: EntryType.LOGIN,
  secret: EntryType.SECRET,
  env: EntryType.ENV,
  file: EntryType.FILE,
  note: EntryType.NOTE,
  ssh_key: EntryType.SSH_KEY,
  database: EntryType.DATABASE,
  api_key: EntryType.API_KEY
} as const;
export function entryTypeFromSlug(slug: string): EntryType {
  return Object.hasOwn(TYPE_BY_SLUG, slug)
    ? TYPE_BY_SLUG[slug as keyof typeof TYPE_BY_SLUG]
    : EntryType.LOGIN;
}
export function entryTypeSlug(type: EntryType): string {
  const slug = Object.entries(TYPE_BY_SLUG).find(([, value]) => value === type)?.[0];
  if (!slug) throw new Error('Unknown entry type');
  return slug;
}
export const ENTRY_TYPES = domainCatalog.entryTypes.map((entry) => entryTypeFromSlug(entry.type));
export const ENTRY_TYPE_LABEL = Object.fromEntries(
  domainCatalog.entryTypes.map((entry) => [entryTypeFromSlug(entry.type), entry.label])
) as Record<EntryType, string>;
export const PRIMARY_COPY_FIELD = Object.fromEntries(
  domainCatalog.entryTypes.map((entry) => [entryTypeFromSlug(entry.type), entry.primaryCopyField])
) as Record<EntryType, string | null>;
export type PayloadOf<T extends Exclude<EntryPayload['data']['case'], undefined>> = Extract<
  EntryPayload['data'],
  { case: T }
>['value'];

export function payloadType(payload: EntryPayload | undefined): EntryType {
  const kind = payload?.data.case;
  if (!kind) return EntryType.UNSPECIFIED;
  return entryTypeFromSlug(kind === 'sshKey' ? 'ssh_key' : kind === 'apiKey' ? 'api_key' : kind);
}
export function emptyPayload(type: EntryType): EntryPayload {
  const slug = entryTypeSlug(type);
  const metadata = domainCatalog.entryTypes.find((entry) => entry.type === slug);
  if (!metadata) throw new Error('Unknown entry type');
  const key = slug === 'ssh_key' ? 'sshKey' : slug === 'api_key' ? 'apiKey' : slug;
  // Catalog defaults use Rust JSON. Protobuf JSON accepts null as an absent optional field.
  return fromJson(EntryPayloadSchema, {
    [key]: metadata.defaultPayload.data as unknown as JsonObject
  });
}
export function emptyEntryInput(profileId: string, type: EntryType): EntryInput {
  return create(EntryInputSchema, { profileId, payload: emptyPayload(type) });
}
export const redactPayload = (payload: EntryPayload) => domain('redactPayload', payload);
export const mergeKeepOnNone = (stored: EntryPayload, input: EntryPayload) =>
  domain('mergeKeepOnNone', { stored, input });
export const mergeEntries = (mine: Entry, theirs: Entry, fromTheirs: ReadonlySet<string>) =>
  domain('mergeEntries', {
    mine: entryToInput(mine),
    theirs: entryToInput(theirs),
    fromTheirs: [...fromTheirs]
  });
export function entryToInput(entry: Entry | EntryInput): EntryInput {
  return create(EntryInputSchema, {
    profileId: entry.profileId,
    name: entry.name,
    tags: [...entry.tags],
    notes: entry.notes,
    favorite: entry.favorite,
    payload: structuredClone(entry.payload)
  });
}

export function formatBytes(size: number | bigint): string {
  size = Number(size);
  if (size < 1024) return `${size} B`;
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`;
  return `${(size / 1024 / 1024).toFixed(1)} MB`;
}
