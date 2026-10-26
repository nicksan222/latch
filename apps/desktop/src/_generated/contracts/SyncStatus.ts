// Generated from the Protobuf descriptor. Do not edit.
import type { CommitInfo } from './CommitInfo';
import type { ConflictSummary } from './ConflictSummary';

export type SyncStatus = { "hasRemote": boolean, "remoteUrl": string | null, "branch": string, "ahead": number, "behind": number, "dirty": boolean, "lastSyncAt": string | null, "conflicts": Array<ConflictSummary>, "log": Array<CommitInfo>, };
