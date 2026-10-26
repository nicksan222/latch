// Generated from the Protobuf descriptor. Do not edit.
import type { BrowserSettings } from './BrowserSettings';
import type { DomainDiffRow } from './DomainDiffRow';
import type { DomainRecipientKeyKind } from './DomainRecipientKeyKind';
import type { EntryInput } from './EntryInput';
import type { EntryPayload } from './EntryPayload';
import type { EnvVar } from './EnvVar';
import type { ParsedOtpauth } from './ParsedOtpauth';
import type { PasswordStrength } from './PasswordStrength';
import type { RedactedPayload } from './RedactedPayload';

export type DomainResponse = { "operation": "validate_entry", "output": { [key in string]?: string } } | { "operation": "validate_profile", "output": { [key in string]?: string } } | { "operation": "validate_recipient", "output": { [key in string]?: string } } | { "operation": "recipient_key_kind", "output": DomainRecipientKeyKind | null } | { "operation": "parse_dotenv", "output": Array<EnvVar> } | { "operation": "format_env", "output": string } | { "operation": "parse_otpauth", "output": ParsedOtpauth | null } | { "operation": "generate_password", "output": string } | { "operation": "estimate_strength", "output": PasswordStrength } | { "operation": "entry_defaults", "output": EntryInput } | { "operation": "database_url", "output": string } | { "operation": "secret_paths", "output": Array<string> } | { "operation": "read_field", "output": string | null } | { "operation": "redact_payload", "output": RedactedPayload } | { "operation": "merge_keep_on_none", "output": EntryPayload } | { "operation": "apply_optional_clears", "output": EntryPayload } | { "operation": "flatten_entry", "output": { [key in string]?: string } } | { "operation": "merge_entries", "output": EntryInput } | { "operation": "default_browser_settings", "output": BrowserSettings } | { "operation": "file_digest", "output": string | null } | { "operation": "diff_rows", "output": Array<DomainDiffRow> };
