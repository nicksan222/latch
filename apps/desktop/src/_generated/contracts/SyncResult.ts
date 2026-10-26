// Generated from the Protobuf descriptor. Do not edit.
import type { ConflictSummary } from './ConflictSummary';
import type { SyncStatus } from './SyncStatus';

export type SyncResult = { "pulled": number, "pushed": number, "conflicts": Array<ConflictSummary>, "status": SyncStatus, };
