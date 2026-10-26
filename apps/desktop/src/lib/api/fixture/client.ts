import {
  create,
  fromBinary,
  toBinary,
  type DescMethodUnary,
  type MessageInitShape,
  type MessageShape
} from '@bufbuild/protobuf';
import { ErrorCode, LatchAgent } from '@latch/rpc/v1';
import { AgentEventSchema } from '@latch/rpc/v1';
import { notify } from '~/lib/notify';
/**
 * Development client for isolated Rust fixture sessions.
 * All state transitions, fixture data and domain behavior run in Rust.
 */
import { RpcError } from '../errors';
import type { EventListener } from '../transport';

export interface FixtureOptions {
  fresh?: boolean;
  unlocked?: boolean;
  latencyMs?: number;
  kdfDelayMs?: number;
  /** Node tests point at their isolated Rust server through LATCH_WEB_BRIDGE_URL. */
  baseUrl?: string;
}

export interface FixtureMetadata {
  passphrase: string;
  profiles: Record<string, string>;
  entries: Record<string, string>;
  vaults: Record<string, string>;
}

export interface FixtureInspection {
  clipboard: string | null;
  lastExport: unknown | null;
}

interface Session {
  sessionId: string;
  fixture: FixtureMetadata;
  inspection: FixtureInspection;
}

export class FixtureClient {
  readonly ready: Promise<void>;
  /** Method names only: request parameters can contain secrets. */
  readonly calls: { method: string }[] = [];
  private session: Session | null = null;
  private readonly listeners = new Set<EventListener>();
  private readonly baseUrl: string;
  private readonly abort = new AbortController();
  private polling = false;
  private disposed = false;
  private disposal: Promise<void> | null = null;

  constructor(options: FixtureOptions = {}) {
    this.baseUrl =
      options.baseUrl ??
      (typeof process !== 'undefined' ? (process.env.LATCH_WEB_BRIDGE_URL ?? '') : '');
    // Do not abort allocation on pagehide: its response contains the capability
    // needed to release it. A pending dispose waits for this response.
    this.ready = this.request<Session>(
      '/session',
      'POST',
      {
        fresh: options.fresh,
        unlocked: options.unlocked,
        latencyMs: options.latencyMs,
        kdfDelayMs: options.kdfDelayMs
      },
      true
    ).then((session) => {
      this.session = session;
    });
  }

  get fixture(): FixtureMetadata {
    if (this.disposed || !this.session)
      throw new Error('Await client.ready before reading Rust fixture metadata');
    return this.session.fixture;
  }

  /** Latest Rust inspection; call inspect() when changes may have happened remotely. */
  get clipboard(): string | null {
    return this.session?.inspection.clipboard ?? null;
  }

  private async request<T>(
    path: string,
    method = 'GET',
    body?: unknown,
    allocating = false
  ): Promise<T> {
    const response = await fetch(`${this.baseUrl}/__latch${path}`, {
      method,
      headers: {
        ...(body === undefined ? {} : { 'Content-Type': 'application/json' }),
        // Node fetch has no browser Origin. The isolated test server requires one.
        ...(this.baseUrl ? { Origin: 'http://127.0.0.1:1420' } : {})
      },
      ...(body === undefined ? {} : { body: JSON.stringify(body) }),
      cache: 'no-store',
      credentials: 'omit',
      ...(allocating ? { keepalive: true } : { signal: this.abort.signal })
    });
    if (!response.ok)
      throw new RpcError(ErrorCode.IO, `Rust backend request failed (${response.status})`);
    return response.status === 204 ? (undefined as T) : ((await response.json()) as T);
  }

  async requestBytes(method: DescMethodUnary, payload: Uint8Array): Promise<Uint8Array> {
    await this.ready;
    if (this.disposed || !this.session)
      throw new RpcError(ErrorCode.IO, 'Rust fixture session is closed');
    this.calls.push({ method: method.name });
    const response = await fetch(`${this.baseUrl}/__latch/rpc`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/proto',
        'x-latch-session': this.session.sessionId,
        'x-latch-method': method.name,
        ...(this.baseUrl ? { Origin: 'http://127.0.0.1:1420' } : {})
      },
      body: new Uint8Array(payload),
      cache: 'no-store',
      credentials: 'omit',
      signal: this.abort.signal
    });
    if (!response.ok) {
      if (response.status === 422) throw RpcError.from(await response.json(), method.name);
      throw new RpcError(ErrorCode.IO, `Rust backend request failed (${response.status})`);
    }
    const bytes = new Uint8Array(await response.arrayBuffer());
    if (
      method === LatchAgent.method.clipboardCopy ||
      method === LatchAgent.method.envCopy ||
      method === LatchAgent.method.exportRun
    )
      await this.inspect();
    return bytes;
  }

  async handle<M extends DescMethodUnary>(
    method: M,
    params: MessageInitShape<M['input']> = {} as MessageInitShape<M['input']>
  ): Promise<MessageShape<M['output']>> {
    const bytes = toBinary(method.input, create(method.input, params));
    return fromBinary(method.output, await this.requestBytes(method, bytes)) as MessageShape<
      M['output']
    >;
  }

  /** Development-only scenario preparation; never part of the product RPC service. */
  async syncScenario(scenario: 'remote_change' | 'conflict'): Promise<void> {
    await this.ready;
    if (this.disposed || !this.session)
      throw new RpcError(ErrorCode.IO, 'Rust fixture session is closed');
    await this.request('/sync-scenario', 'POST', { sessionId: this.session.sessionId, scenario });
  }

  async inspect(): Promise<FixtureInspection> {
    await this.ready;
    if (this.disposed || !this.session)
      throw new RpcError(ErrorCode.IO, 'Rust fixture session is closed');
    const session = this.session;
    const inspection = await this.request<FixtureInspection>('/inspect', 'POST', {
      sessionId: session.sessionId
    });
    if (this.session === session) session.inspection = inspection;
    return inspection;
  }

  subscribe(listener: EventListener): () => void {
    if (this.disposed) return () => {};
    this.listeners.add(listener);
    void this.poll();
    return () => {
      this.listeners.delete(listener);
    };
  }

  private async poll(): Promise<void> {
    if (this.polling || this.disposed) return;
    this.polling = true;
    let connected = false;
    try {
      await this.ready;
      if (this.session && this.listeners.size && !this.abort.signal.aborted) {
        const { events } = await this.request<{ events: number[][] }>('/events', 'POST', {
          sessionId: this.session.sessionId
        });
        for (const event of events)
          for (const listener of this.listeners)
            listener(fromBinary(AgentEventSchema, new Uint8Array(event)));
        connected = true;
      }
    } catch (error) {
      if (!this.abort.signal.aborted) notify.error('Disconnected from Rust backend', error);
    } finally {
      this.polling = false;
      if (connected && this.listeners.size && !this.abort.signal.aborted) void this.poll();
    }
  }

  /** Idempotent for pagehide, explicit test teardown and calls before ready. */
  dispose(): Promise<void> {
    if (this.disposal) return this.disposal;
    this.disposed = true;
    this.listeners.clear();
    this.abort.abort();
    this.disposal = this.release();
    return this.disposal;
  }

  private async release(): Promise<void> {
    await this.ready.catch(() => {});
    const session = this.session;
    this.session = null;
    if (!session) return;
    const response = await fetch(`${this.baseUrl}/__latch/session`, {
      method: 'DELETE',
      headers: {
        'Content-Type': 'application/json',
        ...(this.baseUrl ? { Origin: 'http://127.0.0.1:1420' } : {})
      },
      body: JSON.stringify({ sessionId: session.sessionId }),
      cache: 'no-store',
      credentials: 'omit',
      // The body is tiny; let the browser complete it after the page goes away.
      keepalive: true
    });
    if (!response.ok)
      throw new RpcError(ErrorCode.IO, `Rust session release failed (${response.status})`);
  }
}
