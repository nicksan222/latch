import type { ErrorCode } from '../../_generated/contracts';

const ERROR_CODES: readonly ErrorCode[] = [
  'locked',
  'no_vault',
  'not_found',
  'already_exists',
  'invalid_params',
  'bad_credentials',
  'integrity',
  'conflict',
  'denied',
  'unsupported_version',
  'git',
  'io',
  'internal'
];

export function isErrorCode(value: unknown): value is ErrorCode {
  return typeof value === 'string' && (ERROR_CODES as readonly string[]).includes(value);
}

/** Typed RPC failure. Messages never contain secret values (agent rule). */
export class RpcError extends Error {
  readonly code: ErrorCode;
  readonly method: string | undefined;

  constructor(code: ErrorCode, message: string, method?: string) {
    super(message);
    this.name = 'RpcError';
    this.code = code;
    this.method = method;
  }

  /** Normalizes anything a transport may throw into an RpcError. */
  static from(error: unknown, method?: string): RpcError {
    if (error instanceof RpcError) return error;
    if (error && typeof error === 'object') {
      const record = error as Record<string, unknown>;
      // `{ code, message }` (RpcError on the wire) or `{ error: { code, message } }`.
      const inner =
        record.error && typeof record.error === 'object'
          ? (record.error as Record<string, unknown>)
          : record;
      if (isErrorCode(inner.code)) {
        return new RpcError(inner.code, String(inner.message ?? inner.code), method);
      }
      if (error instanceof Error) return new RpcError('internal', error.message, method);
    }
    if (typeof error === 'string') {
      try {
        return RpcError.from(JSON.parse(error), method);
      } catch {
        return new RpcError('internal', error, method);
      }
    }
    return new RpcError('internal', 'Unknown error', method);
  }
}

export function isRpcError(error: unknown, code?: ErrorCode): error is RpcError {
  return error instanceof RpcError && (code === undefined || error.code === code);
}

const FRIENDLY: Partial<Record<ErrorCode, string>> = {
  locked: 'The vault is locked.',
  no_vault: 'No vault is configured.',
  bad_credentials: 'Wrong passphrase or identity.',
  integrity: 'Integrity check failed. The vault may have been tampered with.',
  conflict: 'Sync conflict needs resolution.',
  denied: 'Not allowed.'
};

export function errorMessage(error: unknown): string {
  if (error instanceof RpcError) return error.message || FRIENDLY[error.code] || error.code;
  if (error instanceof Error) return error.message;
  return String(error);
}
