import type {
  DomainResponse,
  EntryInput,
  ProfileInput,
  RecipientInfo
} from '../_generated/contracts';
import { domain } from './domain';
export type FieldErrors = Extract<DomainResponse, { operation: 'validate_entry' }>['output'];
export const validateEntryInput = (input: EntryInput) => domain('validate_entry', input);
export const validateProfileInput = (input: ProfileInput) => domain('validate_profile', input);
export const recipientKeyKind = (key: string) => domain('recipient_key_kind', key);
export const validateRecipient = (label: string, key: string, existing: readonly RecipientInfo[]) =>
  domain('validate_recipient', { label, key, existing: [...existing] });
