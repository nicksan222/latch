// Generated from the Protobuf descriptor. Do not edit.
import type { CustomField } from './CustomField';
import type { TotpConfig } from './TotpConfig';

export type LoginData = { "username": string, "password": string | null, "urls": Array<string>, "totp": TotpConfig | null, "custom": Array<CustomField>, };
