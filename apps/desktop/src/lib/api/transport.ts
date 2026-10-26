import type { AgentEvent } from '../../_generated/contracts';

export type EventListener = (event: AgentEvent) => void;

/** A way to reach the agent: Tauri IPC in the desktop app, the Rust fixture bridge in development. */
export interface Transport {
  readonly kind: 'tauri' | 'rust' | 'stub';
  call(method: string, params: unknown): Promise<unknown>;
  subscribe(listener: EventListener): () => void;
}

export { isTauri } from '@tauri-apps/api/core';
