import type { EntryInput, ProfileInput, RecipientInfo } from '@latch/rpc/v1';
import { domain } from './domain';
export type FieldErrors = Record<string, string>;
export const validateEntryInput = (input: EntryInput) =>
  domain('validateEntry', input).then((result) => result.items);
export const validateProfileInput = (input: ProfileInput) =>
  domain('validateProfile', input).then((result) => result.items);
export const recipientKeyKind = (key: string) =>
  domain('recipientKeyKind', { value: key }).then((result) => result.value);
export const validateRecipient = (label: string, key: string, existing: readonly RecipientInfo[]) =>
  domain('validateRecipient', { label, key, existing: [...existing] }).then(
    (result) => result.items
  );
