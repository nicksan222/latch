import { fromBinary } from '@bufbuild/protobuf';
import { AgentEventSchema } from '@latch/rpc/v1';
import { invoke } from '@tauri-apps/api/core';
import type { EventListener, Transport } from '../lib/api/transport';

export const TAURI_EVENT = 'latch://event';

export function createTauriTransport(): Transport {
  const listeners = new Set<EventListener>();
  let registration: { unlisten: (() => void) | null } | null = null;
  const ensureListening = () => {
    if (registration) return;
    const pending = { unlisten: null as (() => void) | null };
    registration = pending;
    void import('@tauri-apps/api/event')
      .then(({ listen }) =>
        listen<number[]>(TAURI_EVENT, (event) => {
          const decoded = fromBinary(AgentEventSchema, new Uint8Array(event.payload));
          for (const listener of listeners) listener(decoded);
        })
      )
      .then((unlisten) => {
        if (listeners.size === 0) {
          registration = null;
          unlisten();
        } else pending.unlisten = unlisten;
      })
      .catch(() => {
        registration = null;
      });
  };
  return {
    kind: 'tauri',
    async request(method, payload) {
      return new Uint8Array(
        await invoke<number[]>('grpc_request', {
          method: method.name,
          payload: Array.from(payload)
        })
      );
    },
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
