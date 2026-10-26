import { notify } from '~/lib/notify';
/**
 * Development client for isolated Rust fixture sessions.
 * All state transitions, fixture data and domain behavior run in Rust.
 */
import type { AgentEvent, RpcError as RpcErrorPayload } from '../../../_generated/contracts';
import { RpcError } from '../errors';
import type { EventListener } from '../transport';

// Envelope belongs only to the development fixture HTTP bridge.
interface FixtureResponse {
  id: number;
  result?: unknown;
  error?: RpcErrorPayload;
}

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
  private requestId = 0;
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
    if (!response.ok) throw new RpcError('io', `Rust backend request failed (${response.status})`);
    return response.status === 204 ? (undefined as T) : ((await response.json()) as T);
  }

  async handle(method: string, params: unknown): Promise<unknown> {
    await this.ready;
    if (this.disposed || !this.session) throw new RpcError('io', 'Rust fixture session is closed');
    this.calls.push({ method });
    const { response, inspection } = await this.request<{
      response: FixtureResponse;
      inspection: FixtureInspection;
    }>('/rpc', 'POST', {
      sessionId: this.session.sessionId,
      request: { id: ++this.requestId, method, params }
    });
    if (this.session) this.session.inspection = inspection;
    if (response.error) throw RpcError.from(response.error, method);
    return response.result;
  }

  async inspect(): Promise<FixtureInspection> {
    await this.ready;
    if (this.disposed || !this.session) throw new RpcError('io', 'Rust fixture session is closed');
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
        const { events } = await this.request<{ events: AgentEvent[] }>('/events', 'POST', {
          sessionId: this.session.sessionId
        });
        for (const event of events) for (const listener of this.listeners) listener(event);
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
    if (!response.ok) throw new RpcError('io', `Rust session release failed (${response.status})`);
  }
}
