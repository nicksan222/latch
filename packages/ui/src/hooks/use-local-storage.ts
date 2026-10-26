import { useSyncExternalStore } from 'react';

/** A decoder validates JSON-parsed data; it returns `null`/throws on invalid input. */
export type Decoder<T> = (value: unknown) => T | null;

export const Decoders = {
  finite: ((value: unknown) =>
    typeof value === 'number' && Number.isFinite(value) ? value : null) as Decoder<number>,
  boolean: ((value: unknown) => (typeof value === 'boolean' ? value : null)) as Decoder<boolean>,
  string: ((value: unknown) => (typeof value === 'string' ? value : null)) as Decoder<string>,
  literal:
    <const T extends string>(...values: T[]): Decoder<T> =>
    (value: unknown) =>
      typeof value === 'string' && (values as string[]).includes(value) ? (value as T) : null
};

const fallbackStore = new Map<string, string>();

function storage(): Pick<Storage, 'getItem' | 'setItem' | 'removeItem'> {
  try {
    if (typeof window !== 'undefined' && window.localStorage) return window.localStorage;
  } catch {
    // Access can throw in sandboxed contexts.
  }
  return {
    getItem: (k) => fallbackStore.get(k) ?? null,
    setItem: (k, v) => void fallbackStore.set(k, v),
    removeItem: (k) => void fallbackStore.delete(k)
  };
}

function decode<T>(raw: string | null, decoder: Decoder<T>): T | null {
  if (raw === null) return null;
  try {
    return decoder(JSON.parse(raw));
  } catch {
    return null;
  }
}

export const getLocalStorageItem = <T>(key: string, decoder: Decoder<T>): T | null =>
  decode(storage().getItem(key), decoder);

export const setLocalStorageItem = <T>(key: string, value: T, _decoder?: Decoder<T>) => {
  try {
    storage().setItem(key, JSON.stringify(value));
  } catch (error) {
    console.error('[localStorage] write failed', error);
  }
  notify(key);
};

export const removeLocalStorageItem = (key: string) => {
  storage().removeItem(key);
  notify(key);
};

const CHANGE_EVENT = 'latch:local_storage_change';

function notify(key: string) {
  if (typeof window === 'undefined') return;
  window.dispatchEvent(new CustomEvent<{ key: string }>(CHANGE_EVENT, { detail: { key } }));
}

export function useLocalStorage<T>(
  key: string,
  initialValue: T,
  decoder: Decoder<T>
): [T, (value: T | ((val: T) => T)) => void] {
  const subscribe = (onChange: () => void) => {
    const onStorage = (event: StorageEvent) => event.key === key && onChange();
    const onLocal = (event: Event) =>
      (event as CustomEvent<{ key: string }>).detail.key === key && onChange();
    window.addEventListener('storage', onStorage);
    window.addEventListener(CHANGE_EVENT, onLocal);
    return () => {
      window.removeEventListener('storage', onStorage);
      window.removeEventListener(CHANGE_EVENT, onLocal);
    };
  };
  const getSnapshot = () => storage().getItem(key);
  const raw = useSyncExternalStore(subscribe, getSnapshot, getSnapshot);
  const value = decode(raw, decoder) ?? initialValue;
  const setValue = (next: T | ((val: T) => T)) => {
    const resolved =
      typeof next === 'function'
        ? (next as (val: T) => T)(getLocalStorageItem(key, decoder) ?? initialValue)
        : next;
    if (resolved === null) removeLocalStorageItem(key);
    else setLocalStorageItem(key, resolved);
  };
  return [value, setValue];
}
