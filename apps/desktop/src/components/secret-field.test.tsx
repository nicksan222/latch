import { cleanup, act, renderHook, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';
import { setTransport } from '~/lib/api/client';
import type { FixtureClient } from '~/lib/api/fixture';
import { entryId, renderWithProviders, setupFixture } from '~/test/helpers';
import { SecretField, useReveal } from './secret-field';

let backend: FixtureClient;
let id: string;
const MASK = '••••••••••••';

beforeEach(async () => {
  backend = await setupFixture();
  id = await entryId(backend, 'GitHub');
});
afterEach(async () => {
  cleanup();
  await backend.dispose();
  setTransport(null);
  vi.useRealTimers();
});

describe('useReveal', () => {
  it('drops the value after the timeout', async () => {
    vi.useFakeTimers();
    const { result } = renderHook(() => useReveal(async () => 's3cret', 3));
    await act(async () => {
      await result.current.reveal();
    });
    expect(result.current.value).toBe('s3cret');
    expect(result.current.secondsLeft).toBe(3);
    await act(async () => {
      vi.advanceTimersByTime(2000);
    });
    expect(result.current.revealed).toBe(true);
    await act(async () => {
      vi.advanceTimersByTime(1300);
    });
    expect(result.current.value).toBeNull();
  });

  it('drops the value on manual hide and clears its timer on unmount', async () => {
    const clear = vi.spyOn(globalThis, 'clearInterval');
    const { result, unmount } = renderHook(() => useReveal(async () => 'v', 20));
    await act(async () => {
      await result.current.reveal();
    });
    act(() => result.current.hide());
    expect(result.current.value).toBeNull();
    await act(async () => {
      await result.current.reveal();
    });
    unmount();
    expect(clear).toHaveBeenCalled();
  });
});

function deferred<T>() {
  let resolve!: (v: T) => void;
  let reject!: (e: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

describe('useReveal races', () => {
  it('a response arriving after Hide is discarded', async () => {
    const d = deferred<string>();
    const { result } = renderHook(() => useReveal(() => d.promise, 20));
    let pending!: Promise<void>;
    act(() => {
      pending = result.current.reveal();
    });
    expect(result.current.loading).toBe(true);
    act(() => result.current.hide());
    await act(async () => {
      d.resolve('late-secret');
      await pending;
    });
    expect(result.current.value).toBeNull();
    expect(result.current.revealed).toBe(false);
    expect(result.current.loading).toBe(false);
  });

  it('ignores duplicate reveals while one is pending', async () => {
    const d = deferred<string>();
    const fetch = vi.fn(() => d.promise);
    const { result } = renderHook(() => useReveal(fetch, 20));
    let first!: Promise<void>;
    act(() => {
      first = result.current.reveal();
      void result.current.reveal();
      void result.current.reveal();
    });
    expect(fetch).toHaveBeenCalledTimes(1);
    await act(async () => {
      d.resolve('v1');
      await first;
    });
    expect(result.current.value).toBe('v1');
  });

  it('an old response cannot overwrite a newer reveal (hide → reveal again)', async () => {
    const old = deferred<string>();
    const fresh = deferred<string>();
    const fetch = vi.fn().mockReturnValueOnce(old.promise).mockReturnValueOnce(fresh.promise);
    const { result } = renderHook(() => useReveal<string>(fetch, 20));
    let p1!: Promise<void>;
    let p2!: Promise<void>;
    act(() => {
      p1 = result.current.reveal();
    });
    act(() => result.current.hide());
    act(() => {
      p2 = result.current.reveal();
    });
    await act(async () => {
      fresh.resolve('new');
      await p2;
    });
    await act(async () => {
      old.resolve('old');
      await p1;
    });
    expect(result.current.value).toBe('new');
  });

  it('a response arriving after unmount is dropped', async () => {
    const d = deferred<string>();
    const seen: Array<string | null> = [];
    const { result, unmount } = renderHook(() => {
      const r = useReveal(() => d.promise, 20);
      seen.push(r.value);
      return r;
    });
    let pending!: Promise<void>;
    act(() => {
      pending = result.current.reveal();
    });
    unmount();
    await act(async () => {
      d.resolve('after-unmount');
      await pending;
    });
    expect(seen).not.toContain('after-unmount');
  });

  it('SecretField: clicking Hide before the reveal response arrives keeps the field masked', async () => {
    const user = userEvent.setup();
    const gate = deferred<void>();
    const original = backend.handle.bind(backend);
    vi.spyOn(backend, 'handle').mockImplementation(async (method, params) => {
      if (method === 'entries.reveal') await gate.promise;
      return original(method, params);
    });
    renderWithProviders(
      <SecretField entryId={id} field="password" hideAfterSecs={20} label="Password" />
    );
    const input = screen.getByRole('textbox', { name: 'Password' });
    await user.click(screen.getByRole('button', { name: 'Reveal Password' }));
    // Still loading: the toggle reads "Hide" and cancels the pending reveal.
    await user.click(await screen.findByRole('button', { name: 'Hide Password' }));
    gate.resolve();
    await new Promise((r) => setTimeout(r, 50));
    expect(input).toHaveValue(MASK);
    expect(screen.queryByDisplayValue('hunter2-acme-Gh!')).toBeNull();
  });
});

describe('secret-field', () => {
  it('is masked until revealed, reveals one field, then auto-hides', async () => {
    const user = userEvent.setup();
    renderWithProviders(
      <SecretField entryId={id} field="password" hideAfterSecs={1} label="Password" />
    );
    const input = screen.getByRole('textbox', { name: 'Password' });
    expect(input).toHaveValue(MASK);
    expect(backend.calls.some((c) => c.method === 'entries.reveal')).toBe(false);

    await user.click(screen.getByRole('button', { name: 'Reveal Password' }));
    await waitFor(() => expect(input).toHaveValue('hunter2-acme-Gh!'));
    expect(backend.calls.filter((c) => c.method === 'entries.reveal')).toEqual([
      { method: 'entries.reveal' }
    ]);
    await waitFor(() => expect(input).toHaveValue(MASK), { timeout: 2500 });
  });

  it('copies through clipboard.copy without fetching plaintext', async () => {
    const user = userEvent.setup();
    renderWithProviders(
      <SecretField entryId={id} field="password" hideAfterSecs={20} label="Password" />
    );
    await user.click(screen.getByRole('button', { name: 'Copy Password' }));
    await waitFor(() => expect(backend.clipboard).toBe('hunter2-acme-Gh!'));
    expect(backend.calls.map((c) => c.method)).not.toContain('entries.reveal');
    expect(screen.getByRole('textbox', { name: 'Password' })).toHaveValue(MASK);
  });

  it('is keyboard complete: Space toggles reveal, Enter copies', async () => {
    const user = userEvent.setup();
    renderWithProviders(
      <SecretField entryId={id} field="custom.Recovery" hideAfterSecs={20} label="Recovery" />
    );
    const input = screen.getByRole('textbox', { name: 'Recovery' });
    input.focus();
    await user.keyboard(' ');
    await waitFor(() => expect(input).toHaveValue('abcd-efgh-ijkl'));
    await user.keyboard(' ');
    expect(input).toHaveValue(MASK);
    await user.keyboard('{Enter}');
    await waitFor(() => expect(backend.clipboard).toBe('abcd-efgh-ijkl'));
  });

  it('drops revealed plaintext on unmount (navigation)', async () => {
    const user = userEvent.setup();
    const { unmount } = renderWithProviders(
      <SecretField entryId={id} field="password" hideAfterSecs={20} label="Password" />
    );
    await user.click(screen.getByRole('button', { name: 'Reveal Password' }));
    await screen.findByDisplayValue('hunter2-acme-Gh!');
    unmount();
    expect(screen.queryByDisplayValue('hunter2-acme-Gh!')).toBeNull();
  });

  it('renders a dash for empty secrets', () => {
    renderWithProviders(
      <SecretField
        empty
        entryId={id}
        field="passphrase"
        hideAfterSecs={20}
        label="Key passphrase"
      />
    );
    expect(screen.queryByRole('textbox')).toBeNull();
    expect(screen.getByText('—')).toBeInTheDocument();
  });
});
