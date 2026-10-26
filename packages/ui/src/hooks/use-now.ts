import { useEffect, useState } from 'react';

/** Re-renders every `intervalMs` with the current time (aligned to the second). */
export function useNow(intervalMs = 1000, enabled = true): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!enabled) return;
    let timeout: ReturnType<typeof setTimeout>;
    const tick = () => {
      setNow(Date.now());
      timeout = setTimeout(tick, intervalMs - (Date.now() % intervalMs));
    };
    timeout = setTimeout(tick, intervalMs - (Date.now() % intervalMs));
    return () => clearTimeout(timeout);
  }, [intervalMs, enabled]);
  return now;
}
