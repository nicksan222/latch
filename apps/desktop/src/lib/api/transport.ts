import { create, fromBinary, toBinary, type DescMethodUnary } from '@bufbuild/protobuf';
import type { Transport as ConnectTransport } from '@connectrpc/connect';
import type { AgentEvent } from '@latch/rpc/v1';

export type EventListener = (event: AgentEvent) => void;

/** App adapters carry encoded protobuf messages; method identity comes from the service descriptor. */
export interface Transport {
  readonly kind: 'tauri' | 'rust' | 'stub';
  request(method: DescMethodUnary, payload: Uint8Array): Promise<Uint8Array>;
  subscribe(listener: EventListener): () => void;
}

export function connectTransport(adapter: Transport): ConnectTransport {
  return {
    async unary(method, _signal, _timeout, _header, input) {
      const bytes = await adapter.request(
        method,
        toBinary(method.input, create(method.input, input))
      );
      return {
        stream: false,
        service: method.parent,
        method,
        header: new Headers(),
        trailer: new Headers(),
        message: fromBinary(method.output, bytes)
      };
    },
    async stream() {
      throw new Error('Agent events use the app event subscription');
    }
  };
}
export { isTauri } from '@tauri-apps/api/core';
