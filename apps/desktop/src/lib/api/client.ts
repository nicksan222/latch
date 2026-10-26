import { createClient } from '@connectrpc/connect';
import { ErrorCode } from '@latch/rpc/v1';
import { LatchAgent } from '@latch/rpc/v1';
import type { AgentEvent } from '@latch/rpc/v1';
import { RpcError } from './errors';
import type { Method, ParamsArg, ParamsOf, ResultOf } from './methods';
import { connectTransport, isTauri, type EventListener, type Transport } from './transport';

let transportPromise: Promise<Transport> | null = null;
const eventListeners = new Set<EventListener>();
let detach: (() => void) | null = null;

/** Desktop supplies its SDK transport; browser development uses the Rust HTTP bridge. */
export async function selectTransport(): Promise<Transport> {
  if (isTauri()) throw new RpcError(ErrorCode.IO, 'Desktop SDK transport has not been configured');
  if (import.meta.env.DEV) {
    const { createFixtureTransport } = await import('./fixture');
    return createFixtureTransport();
  }
  throw new RpcError(ErrorCode.IO, 'Desktop IPC is unavailable');
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
  const params = args[0] ?? {};
  const client = createClient(LatchAgent, connectTransport(transport));
  try {
    const invoke = client[method.localName as keyof typeof client] as unknown as (
      input: ParamsOf<K>
    ) => Promise<ResultOf<K>>;
    return await invoke(params as ParamsOf<K>);
  } catch (error) {
    throw RpcError.from(error, method.name);
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
