import { useEffect, useLayoutEffect, useRef, useState } from 'react';

/** Runs a UI action only while its component and input snapshot are current. */
export function useAsyncAction(scope: unknown) {
  const current = useRef({ scope, mounted: true, request: 0 });
  useLayoutEffect(() => {
    current.current.scope = scope;
  }, [scope]);
  const [activeRequest, setActiveRequest] = useState<number | null>(null);
  useEffect(() => {
    const state = current.current;
    state.mounted = true;
    return () => {
      state.mounted = false;
      state.request++;
    };
  }, []);
  const run = async <T>(
    work: () => Promise<T>,
    apply: (result: T) => void,
    onError: (error: unknown) => void
  ) => {
    const state = current.current;
    const request = ++state.request;
    const snapshot = state.scope;
    const alive = () => state.mounted && state.request === request;
    const valid = () => alive() && Object.is(state.scope, snapshot);
    setActiveRequest(request);
    try {
      const result = await work();
      if (valid()) apply(result);
    } catch (error) {
      if (valid()) onError(error);
    } finally {
      if (alive()) setActiveRequest(null);
    }
  };
  return { pending: activeRequest !== null, run };
}

/** Transient async presentation result; inputs never enter a query cache or key. */
export function useAsyncValue<I, O>(input: I, load: (input: I) => Promise<O>) {
  const [result, setResult] = useState<{ input: I; value?: O; error?: unknown } | null>(null);
  useEffect(() => {
    let active = true;
    void load(input).then(
      (value) => {
        if (active) setResult({ input, value });
      },
      (error: unknown) => {
        if (active) setResult({ input, error });
      }
    );
    return () => {
      active = false;
    };
  }, [input, load]);
  return result && Object.is(result.input, input) ? result : null;
}
