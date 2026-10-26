import { expect, test as base, type Response } from '@playwright/test';

/** Every browser flow uses the real Rust bridge and releases all allocated sessions. */
export const test = base.extend<{ rustFixtureSessions: void }>({
  rustFixtureSessions: [
    async ({ context }, runTest) => {
      // Keep capabilities only in this test's memory, never diagnostics or URLs.
      const sessions = new Map<string, string>();
      const pending = new Set<Promise<void>>();
      let captureFailed = false;
      const capture = (response: Response) => {
        const url = new URL(response.url());
        if (
          url.pathname !== '/__latch/session' ||
          response.request().method() !== 'POST' ||
          !response.ok()
        )
          return;
        const work = response
          .json()
          .then((body: unknown) => {
            if (
              !body ||
              typeof body !== 'object' ||
              !('sessionId' in body) ||
              typeof body.sessionId !== 'string'
            ) {
              captureFailed = true;
              return;
            }
            sessions.set(body.sessionId, url.origin);
          })
          .catch(() => {
            captureFailed = true;
          });
        pending.add(work);
        void work.finally(() => pending.delete(work));
      };
      context.on('response', capture);
      try {
        await runTest();
      } finally {
        // Await the controller before closing pages, including allocations still
        // in flight. The request-context fallback also handles pages already closed.
        for (const page of context.pages()) {
          await page
            .evaluate(async () => {
              await window.__latchFixture?.dispose();
            })
            .catch(() => {});
        }
        while (pending.size) await Promise.all(pending);
        context.off('response', capture);
        const failures: number[] = [];
        const allocatedSessions = sessions.size;
        for (const [sessionId, origin] of sessions) {
          const options = { headers: { Origin: origin }, data: { sessionId } };
          try {
            const released = await context.request.delete(`${origin}/__latch/session`, options);
            // An earlier pagehide may already have revoked this capability.
            if (released.status() !== 204 && released.status() !== 403)
              failures.push(released.status());
            const inspection = await context.request.post(`${origin}/__latch/inspect`, options);
            if (inspection.status() !== 403) failures.push(inspection.status());
          } catch {
            // Do not include a request body/capability in failure diagnostics.
            failures.push(0);
          }
        }
        sessions.clear();
        expect(
          allocatedSessions,
          'Every flow must allocate a real Rust fixture session'
        ).toBeGreaterThan(0);
        expect(captureFailed, 'Rust session responses must be inspectable for teardown').toBe(
          false
        );
        expect(failures, 'Rust must reject every released session capability').toEqual([]);
      }
    },
    { auto: true }
  ]
});

export { expect };
