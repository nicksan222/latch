// Generated from the Protobuf descriptor. Do not edit.
import type { RecipientInfo } from './RecipientInfo';
import type { TrustStatus } from './TrustStatus';

export type VaultStatus = { "initialized": boolean, "locked": boolean, "vaultId": string | null, "path": string | null, "recipients": Array<RecipientInfo>, "dirty": boolean, "ahead": number, "behind": number, "hasRemote": boolean, "lastSyncAt": string | null, "locksInSecs": number | null, "trust": TrustStatus | null, "integrityProblems": Array<string>, "rotationPending": boolean, "memoryLocked": boolean | null, };
