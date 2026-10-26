import { expect, test } from '../../fixtures';

for (const navigation of ['reload', 'leave'] as const) {
  test(`Rust session is revoked when the page ${navigation === 'reload' ? 'reloads' : 'navigates away'}`, async ({
    page,
    context
  }) => {
    const allocation = page.waitForResponse(
      (response) =>
        new URL(response.url()).pathname === '/__latch/session' &&
        response.request().method() === 'POST' &&
        response.ok()
    );
    await page.goto('/?fresh&fast');
    await expect(page.getByRole('heading', { name: 'Welcome' })).toBeVisible();
    const response = await allocation;
    const { sessionId } = (await response.json()) as { sessionId: unknown };
    if (typeof sessionId !== 'string') throw new Error('Rust session capability missing');
    const origin = new URL(response.url()).origin;
    if (navigation === 'reload') {
      await page.reload();
      await expect(page.getByRole('heading', { name: 'Welcome' })).toBeVisible();
    } else {
      await page.goto('about:blank');
    }
    // The capability stays in a request body. Await actual Rust rejection, not
    // a client-side flag, so this catches a missing pagehide/keepalive release.
    await expect
      .poll(async () => {
        const inspection = await context.request.post(`${origin}/__latch/inspect`, {
          headers: { Origin: origin },
          data: { sessionId }
        });
        return inspection.status();
      })
      .toBe(403);
  });
}
