// Generated from the Protobuf descriptor. Do not edit.
import type { DatabaseData } from './DatabaseData';
import type { DiffRowsInput } from './DiffRowsInput';
import type { DomainEnvFormatInput } from './DomainEnvFormatInput';
import type { EntryDefaultsInput } from './EntryDefaultsInput';
import type { EntryInput } from './EntryInput';
import type { EntryPayload } from './EntryPayload';
import type { MergeEntriesInput } from './MergeEntriesInput';
import type { PayloadMergeInput } from './PayloadMergeInput';
import type { ProfileInput } from './ProfileInput';
import type { ReadFieldInput } from './ReadFieldInput';
import type { ValidateRecipientInput } from './ValidateRecipientInput';

export type DomainRequest = { "operation": "validate_entry", "input": EntryInput } | { "operation": "validate_profile", "input": ProfileInput } | { "operation": "validate_recipient", "input": ValidateRecipientInput } | { "operation": "recipient_key_kind", "input": string } | { "operation": "parse_dotenv", "input": string } | { "operation": "format_env", "input": DomainEnvFormatInput } | { "operation": "parse_otpauth", "input": string } | { "operation": "generate_password", "input": number } | { "operation": "estimate_strength", "input": string } | { "operation": "entry_defaults", "input": EntryDefaultsInput } | { "operation": "database_url", "input": DatabaseData } | { "operation": "secret_paths", "input": EntryPayload } | { "operation": "read_field", "input": ReadFieldInput } | { "operation": "redact_payload", "input": EntryPayload } | { "operation": "merge_keep_on_none", "input": PayloadMergeInput } | { "operation": "apply_optional_clears", "input": EntryPayload } | { "operation": "flatten_entry", "input": EntryInput } | { "operation": "merge_entries", "input": MergeEntriesInput } | { "operation": "default_browser_settings", "input": null } | { "operation": "file_digest", "input": EntryInput | null } | { "operation": "diff_rows", "input": DiffRowsInput };
