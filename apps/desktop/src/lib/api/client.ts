import type { AgentEvent } from '../../_generated/contracts';
import { RpcError } from './errors';
import type { Method, ParamsArg, ResultOf } from './methods';
import { isTauri, type EventListener, type Transport } from './transport';

let transportPromise: Promise<Transport> | null = null;
const eventListeners = new Set<EventListener>();
let detach: (() => void) | null = null;

/** Desktop supplies its SDK transport; browser development uses the Rust HTTP bridge. */
export async function selectTransport(): Promise<Transport> {
  if (isTauri()) throw new RpcError('io', 'Desktop SDK transport has not been configured');
  if (import.meta.env.DEV) {
    const { createFixtureTransport } = await import('./fixture');
    return createFixtureTransport();
  }
  throw new RpcError('io', 'Desktop IPC is unavailable');
}

function attach(transport: Transport) {
  detach?.();
  detach = transport.subscribe((event) => {
    for (const listener of eventListeners) listener(event);
  });
}

export function getTransport(): Promise<Transport> {
  if (!transportPromise) {
    transportPromise = selectTransport().then((transport) => {
      attach(transport);
      return transport;
    });
  }
  return transportPromise;
}

/** Install an app-owned transport before mounting, or replace a development fixture session. */
export function setTransport(transport: Transport | null) {
  detach?.();
  detach = null;
  transportPromise = transport ? Promise.resolve(transport) : null;
  if (transport) attach(transport);
}

/** Typed RPC call. Rejects with {@link RpcError}. */
export async function call<K extends Method>(
  method: K,
  ...args: ParamsArg<K>
): Promise<ResultOf<K>> {
  const transport = await getTransport();
  const params = args.length > 0 ? args[0] : null;
  try {
    return (await transport.call(method, params)) as ResultOf<K>;
  } catch (error) {
    throw RpcError.from(error, method);
  }
}

/** Subscribe to agent events (`latch://event`). */
export function onAgentEvent(listener: (event: AgentEvent) => void): () => void {
  eventListeners.add(listener);
  void getTransport();
  return () => {
    eventListeners.delete(listener);
  };
}
