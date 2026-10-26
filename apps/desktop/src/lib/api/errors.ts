import { ErrorCode } from '@latch/rpc/v1';

export function isErrorCode(value: unknown): value is ErrorCode {
  return typeof value === 'number' && value > ErrorCode.UNSPECIFIED && value <= ErrorCode.INTERNAL;
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
      if (error instanceof Error) return new RpcError(ErrorCode.INTERNAL, error.message, method);
    }
    if (typeof error === 'string') {
      try {
        return RpcError.from(JSON.parse(error), method);
      } catch {
        return new RpcError(ErrorCode.INTERNAL, error, method);
      }
    }
    return new RpcError(ErrorCode.INTERNAL, 'Unknown error', method);
  }
}

export function isRpcError(error: unknown, code?: ErrorCode): error is RpcError {
  return error instanceof RpcError && (code === undefined || error.code === code);
}

const FRIENDLY: Partial<Record<ErrorCode, string>> = {
  [ErrorCode.LOCKED]: 'The vault is locked.',
  [ErrorCode.NO_VAULT]: 'No vault is configured.',
  [ErrorCode.BAD_CREDENTIALS]: 'Wrong passphrase or identity.',
  [ErrorCode.INTEGRITY]: 'Integrity check failed. The vault may have been tampered with.',
  [ErrorCode.CONFLICT]: 'Sync conflict needs resolution.',
  [ErrorCode.DENIED]: 'Not allowed.'
};

export function errorMessage(error: unknown): string {
  if (error instanceof RpcError) return error.message || FRIENDLY[error.code] || 'Unknown error';
  if (error instanceof Error) return error.message;
  return String(error);
}
