// Generated from the Protobuf descriptor. Do not edit.
import type { ApiKeyData } from './ApiKeyData';
import type { DatabaseData } from './DatabaseData';
import type { EnvData } from './EnvData';
import type { FileData } from './FileData';
import type { LoginData } from './LoginData';
import type { NoteData } from './NoteData';
import type { SecretData } from './SecretData';
import type { SshKeyData } from './SshKeyData';

export type EntryPayload = { "type": "login", "data": LoginData } | { "type": "secret", "data": SecretData } | { "type": "env", "data": EnvData } | { "type": "file", "data": FileData } | { "type": "note", "data": NoteData } | { "type": "ssh_key", "data": SshKeyData } | { "type": "database", "data": DatabaseData } | { "type": "api_key", "data": ApiKeyData };
