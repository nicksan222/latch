import type { AgentEvent } from '../_generated/contracts';
import type { Transport } from '../lib/api/transport';
import { createDesktopSdk } from './generated';

type EventListener = (event: AgentEvent) => void;

export const TAURI_EVENT = 'latch://event';

/**
 * Desktop transport: `invoke("rpc", { method, params })`. The command resolves
 * with the method result and rejects with the wire `RpcError` `{ code, message }`.
 * Events arrive as the Tauri event `latch://event` carrying an `AgentEvent`.
 */
export function createTauriTransport(): Transport {
  const sdk = createDesktopSdk();
  const listeners = new Set<EventListener>();
  let registration: { unlisten: (() => void) | null } | null = null;

  const ensureListening = () => {
    if (registration) return;
    // Keep pending registration reusable if subscribers leave and return before it resolves.
    const pending = { unlisten: null as (() => void) | null };
    registration = pending;
    void import('@tauri-apps/api/event')
      .then(({ listen }) =>
        listen<AgentEvent>(TAURI_EVENT, (event) => {
          for (const listener of listeners) listener(event.payload);
        })
      )
      .then((unlisten) => {
        if (listeners.size === 0) {
          registration = null;
          unlisten();
        } else {
          pending.unlisten = unlisten;
        }
      })
      .catch(() => {
        // A later subscription can retry if native registration failed.
        registration = null;
      });
  };

  return {
    kind: 'tauri',
    call: sdk.call,
    subscribe(listener) {
      listeners.add(listener);
      ensureListening();
      return () => {
        listeners.delete(listener);
        if (listeners.size === 0 && registration?.unlisten) {
          const { unlisten } = registration;
          registration = null;
          unlisten();
        }
      };
    }
  };
}
