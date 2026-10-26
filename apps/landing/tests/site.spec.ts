import { test, expect } from '@playwright/test';
import { mkdir } from 'node:fs/promises';
const shots = '../../.impeccable/review';
test('homepage scrolls, has latest builds, and switches project context', async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto('/');
  await expect(page.getByRole('link', { name: 'Download latest' })).toHaveAttribute('href', /\/releases\/latest$/);
  await page.getByRole('button', { name: 'personal', exact: true }).click();
  await expect(page.locator('.context-terminal')).toContainText('personal');
  await expect(page.locator('.context-browser')).toContainText('personal');
  await page.mouse.wheel(0, 900);
  await expect.poll(() => page.evaluate(() => scrollY)).toBeGreaterThan(300);
  await page.evaluate(() => scrollTo({ top: 0, behavior: 'instant' }));
  await page.getByRole('button', { name: 'acme', exact: true }).click();
  await expect(page.locator('.latch-terminal')).toContainText('Run with a profile / acme');
  await page.locator('.live-terminal-demo').scrollIntoViewIfNeeded();
  await expect(page.locator('.latch-terminal-status')).toHaveText('Complete', { timeout: 10000 });
  await page.evaluate(() => document.fonts.ready);
  await page.evaluate(() => scrollTo({ top: 0, behavior: 'instant' }));
  await mkdir(shots, { recursive: true });
  await page.screenshot({ path: `${shots}/desktop.png`, fullPage: true, animations: 'disabled' });
});
test('README navigation and saved theme work', async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto('/');
  await expect(page.getByRole('navigation', { name: 'Main navigation' }).getByRole('link', { name: 'README' })).toHaveAttribute('href', 'https://github.com/nicksan222/latch#readme');
  await expect(page.getByRole('link', { name: 'Read the README' })).toHaveAttribute('href', 'https://github.com/nicksan222/latch#readme');
  await page.getByRole('button', { name: 'Switch to light theme' }).click();
  await page.reload();
  await expect(page.locator('html')).not.toHaveClass(/dark/);
  await page.getByRole('button', { name: 'Switch to dark theme' }).click();
  await expect(page.getByRole('button', { name: 'Search documentation' })).toHaveCount(0);
  await page.screenshot({ path: `${shots}/readme-navigation-desktop.png`, fullPage: true, animations: 'disabled' });
});
test('mobile home keeps scrolling and links documentation to the README without overflow', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto('/');
  await expect(page.locator('.brand-icon')).toBeVisible();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.locator('.latch-terminal').scrollIntoViewIfNeeded();
  await expect(page.locator('.latch-terminal-status')).toHaveText('Complete', { timeout: 10000 });
  await page.evaluate(() => document.fonts.ready);
  await page.evaluate(() => scrollTo({ top: 0, behavior: 'instant' }));
  await page.screenshot({ path: `${shots}/mobile.png`, fullPage: true, animations: 'disabled' });
  await page.mouse.wheel(0, 800);
  await expect.poll(() => page.evaluate(() => scrollY)).toBeGreaterThan(300);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await expect(page.getByRole('navigation', { name: 'Main navigation' }).getByRole('link', { name: 'README' })).toHaveAttribute('href', 'https://github.com/nicksan222/latch#readme');
  await page.screenshot({ path: `${shots}/readme-navigation-mobile.png`, fullPage: true, animations: 'disabled' });
});

for (const width of [320, 768, 1280]) {
  test(`project map fits at ${width}px in both themes`, async ({ page }) => {
    await page.setViewportSize({ width, height: 900 });
    await page.goto('/');
    await page.getByRole('button', { name: 'side-project', exact: true }).click();
    await expect(page.locator('.context-terminal')).toContainText('side-project');
    await expect(page.locator('.context-browser')).toContainText('side-project');
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    await page.getByRole('button', { name: 'Switch to light theme' }).click();
    await expect(page.locator('html')).not.toHaveClass(/dark/);
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  });
}

test('terminal streams real recorded output and supports pause, resume, replay and scenarios', async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto('/');
  const terminal = page.locator('.latch-terminal');
  await terminal.scrollIntoViewIfNeeded();
  await expect(terminal.getByRole('button', { name: 'Pause terminal playback' })).toBeEnabled();
  await expect(terminal.locator('.latch-terminal-lines')).toContainText('acme/runtime', { timeout: 7000 });
  await terminal.getByRole('button', { name: 'Pause terminal playback' }).click();
  await expect(terminal.getByRole('status')).toHaveText('Paused');
  const progress = await terminal.getByRole('progressbar').getAttribute('value');
  await page.waitForTimeout(240);
  await expect(terminal.getByRole('progressbar')).toHaveAttribute('value', progress!);
  await terminal.getByRole('button', { name: 'Resume terminal playback' }).click();
  await expect(terminal.getByRole('status')).toHaveText('Complete', { timeout: 10000 });
  await expect(terminal.locator('.latch-terminal-lines')).toContainText('API_TOKEN available to child process');
  await terminal.getByRole('button', { name: 'Replay terminal demo' }).click();
  await expect(terminal.getByRole('status')).toHaveText('Playing');
  await page.getByRole('button', { name: 'Review a change', exact: true }).click();
  await expect(terminal).toContainText('Review a change');
  await expect(terminal.getByRole('status')).toHaveText('Complete', { timeout: 10000 });
  await expect(terminal.locator('.latch-terminal-lines')).toContainText('latch history acme/runtime');
  await expect(terminal.locator('.latch-terminal-lines')).not.toContainText('API_TOKEN available to child process');
});

test('reduced motion presents full recordings without animated typing', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.goto('/');
  const terminal = page.locator('.latch-terminal');
  await terminal.scrollIntoViewIfNeeded();
  await expect(terminal.getByRole('status')).toHaveText('Complete');
  await expect(terminal.locator('.latch-terminal-lines')).toContainText('API_TOKEN available to child process');
  await expect(terminal.getByRole('button', { name: 'Pause terminal playback' })).toBeDisabled();
  await page.getByRole('button', { name: 'Inspect & lock', exact: true }).click();
  await expect(terminal.locator('.latch-terminal-lines')).toContainText('state:   locked');
});

test('terminal playback suspends while outside the viewport', async ({ page }) => {
  await page.goto('/');
  const terminal = page.locator('.latch-terminal');
  await terminal.scrollIntoViewIfNeeded();
  await expect(terminal.getByRole('status')).toHaveText('Playing');
  await page.locator('.site-footer').scrollIntoViewIfNeeded();
  await expect(terminal.getByRole('status')).toHaveText('Ready');
  const progress = await terminal.getByRole('progressbar').getAttribute('value');
  await page.waitForTimeout(240);
  await expect(terminal.getByRole('progressbar')).toHaveAttribute('value', progress!);
  await terminal.scrollIntoViewIfNeeded();
  await expect(terminal.getByRole('status')).toHaveText('Playing');
});

test('terminal repeats its recorded session and the loop can be paused', async ({ page }) => {
  await page.goto('/');
  const terminal = page.locator('.latch-terminal');
  await terminal.scrollIntoViewIfNeeded();
  await expect(terminal.getByRole('button', { name: 'Pause terminal playback' })).toBeEnabled();
  await expect(terminal.getByRole('status')).toHaveText('Complete', { timeout: 10000 });
  await expect(terminal.getByRole('button', { name: 'Pause terminal playback' })).toBeEnabled();
  await terminal.getByRole('button', { name: 'Pause terminal playback' }).click();
  await expect(terminal.getByRole('status')).toHaveText('Paused');
  await page.waitForTimeout(2900);
  await expect(terminal.getByRole('status')).toHaveText('Paused');
  await terminal.getByRole('button', { name: 'Resume terminal playback' }).click();
  await expect(terminal.getByRole('status')).toHaveText('Playing', { timeout: 5000 });
});

test('reading earlier terminal output stops auto-follow until explicitly resumed', async ({ page }) => {
  await page.goto('/');
  const terminal = page.locator('.latch-terminal');
  await terminal.scrollIntoViewIfNeeded();
  await page.getByRole('button', { name: 'Inspect & lock', exact: true }).click();
  await terminal.scrollIntoViewIfNeeded();
  await expect(terminal.getByRole('status')).toHaveText('Playing');
  await expect(terminal.locator('.latch-terminal-lines')).toContainText('latch lock', { timeout: 10000 });
  const viewport = terminal.locator('[data-slot="scroll-area-viewport"]');
  await viewport.evaluate((element) => { element.scrollTop = 0; element.dispatchEvent(new Event('scroll')); });
  await expect(terminal.locator('.latch-terminal-lines')).toContainText('state:   locked', { timeout: 10000 });
  expect(await viewport.evaluate((element) => element.scrollTop)).toBe(0);
  await terminal.getByRole('button', { name: 'Follow output', exact: true }).click();
  await expect.poll(() => viewport.evaluate((element) => element.scrollTop)).toBeGreaterThan(0);
});

test('embedded app reuses real fields with isolated fictional data', async ({ page }) => {
  const bridgeRequests: string[] = [];
  page.on('request', (request) => { if (/\/rpc|\/api\/|\/session/.test(request.url())) bridgeRequests.push(request.url()); });
  await page.goto('/');
  await page.locator('.hero-demo').scrollIntoViewIfNeeded();
  const demo = page.frameLocator('iframe[title="Interactive Latch app demo"]');
  await expect(demo.getByRole('heading', { name: 'GitHub', exact: true })).toBeVisible();
  await demo.getByRole('button', { name: 'Reveal Password', exact: true }).click();
  await expect(demo.getByLabel('Password', { exact: true })).toHaveValue('demo-only-acme-password');
  await demo.getByTestId('profile-personal').click();
  await expect(demo.getByText('Select an entry', { exact: true })).toBeVisible();
  await demo.getByRole('searchbox', { name: 'Filter entries' }).fill('Postgres');
  await expect(demo.getByRole('link', { name: /GitHub/ })).toHaveCount(0);
  await demo.getByRole('link', { name: /Postgres development/ }).click();
  await expect(demo.getByRole('heading', { name: 'Postgres development' })).toBeVisible();
  await demo.getByRole('button', { name: 'Reset demo' }).click();
  await expect(demo.getByRole('heading', { name: 'GitHub', exact: true })).toBeVisible();
  await expect(demo.getByLabel('Password', { exact: true })).not.toHaveValue('demo-only-acme-password');
  await page.getByRole('button', { name: 'Switch to light theme' }).click();
  await expect(demo.locator('html')).not.toHaveClass(/dark/);
  expect(bridgeRequests).toEqual([]);
});

test('embedded demo supports mobile entry navigation without overflow', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto('/');
  await page.locator('.hero-demo').scrollIntoViewIfNeeded();
  const demo = page.frameLocator('.hero-demo');
  await demo.getByRole('button', { name: 'All entries' }).click();
  await expect(demo.getByRole('searchbox', { name: 'Filter entries' })).toBeVisible();
  await demo.getByRole('link', { name: /Project runbook/ }).focus();
  await page.keyboard.press('Enter');
  await expect(demo.getByRole('heading', { name: 'Project runbook', exact: true })).toBeVisible();
  await demo.getByRole('button', { name: 'Toggle sidebar' }).click();
  await demo.getByTestId('profile-personal').click();
  await page.keyboard.press('Escape');
  await demo.getByRole('link', { name: /Project runbook/ }).click();
  await expect(demo.getByRole('heading', { name: 'Project runbook', exact: true })).toBeVisible();
  expect(await demo.locator('body').evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
});

for (const width of [320, 390, 768, 1024, 1440, 1920]) {
  test(`real app preview and landing fit ${width}px in both themes`, async ({ page }) => {
    await page.setViewportSize({ width, height: 1000 });
    const errors: string[] = [];
    page.on('pageerror', (error) => errors.push(error.message));
    await page.goto('/');
    const frame = page.locator('.hero-demo');
    await frame.scrollIntoViewIfNeeded();
    const demo = page.frameLocator('.hero-demo');
    await expect(demo.getByTestId('entry-detail')).toBeVisible();
    const bounds = await frame.boundingBox();
    expect(bounds!.width).toBeGreaterThanOrEqual(Math.min(1600, width - 64) - 1);
    if (width >= 1440) expect(bounds!.width).toBeGreaterThan(1240);
    for (const light of [false, true]) {
      if (light) await page.getByRole('button', { name: 'Switch to light theme' }).click();
      await expect(demo.locator('html')).toHaveClass(light ? /^(?!.*dark)/ : /dark/);
      expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
      expect(await demo.locator('body').evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
      const overlaps = await demo.locator('header').evaluate((header) => {
        const title = header.querySelector('h1')?.getBoundingClientRect();
        const action = Array.from(header.querySelectorAll('button')).find((button) => button.textContent?.includes('Browser'))?.getBoundingClientRect();
        return title && action ? title.width < 20 || (title.left < action.right && title.right > action.left && title.top < action.bottom && title.bottom > action.top) : false;
      });
      expect(overlaps).toBe(false);
      await page.locator('.home-guide-index').scrollIntoViewIfNeeded();
      await expect(page.getByRole('navigation', { name: 'Explore the project' })).toBeVisible();
    }
    expect(errors).toEqual([]);
  });
}

test('real app search, type filters, history and lock remain interactive', async ({ page }) => {
  await page.setViewportSize({ width: 1600, height: 1000 });
  await page.goto('/demo/');
  await expect(page.getByTestId('entry-detail')).toBeVisible();
  await page.getByRole('tab', { name: 'History', exact: true }).click();
  await expect(page.getByRole('list', { name: 'Entry history' })).toContainText('Update encrypted entry');
  await page.getByRole('button', { name: 'Search entries', exact: true }).click();
  const palette = page.getByRole('dialog', { name: 'Quick search' });
  await palette.getByRole('combobox').fill('Deployment');
  await expect(palette).toContainText('Deployment token');
  await page.keyboard.press('Escape');
  await page.getByRole('button', { name: 'Env', exact: true }).click();
  await expect(page.getByTestId('entry-row')).toHaveCount(1);
  await page.getByRole('link', { name: /Development .env/ }).click();
  await expect(page.getByTestId('entry-detail')).toContainText('DATABASE_URL');
  await page.getByRole('button', { name: 'Lock vault', exact: true }).click();
  await expect(page.getByTestId('lock-screen')).toBeVisible();
  await page.getByRole('button', { name: 'Reset demo', exact: true }).click();
  await expect(page.getByTestId('entry-detail')).toContainText('GitHub');
});

for (const width of [390, 1440]) {
  test(`scrolling over the ${width}px app preview scrolls the landing page`, async ({ page }) => {
    await page.setViewportSize({ width, height: 900 });
    await page.goto('/');
    const iframe = page.locator('.hero-demo');
    await iframe.scrollIntoViewIfNeeded();
    const demo = page.frameLocator('.hero-demo');
    await expect(demo.getByTestId('entry-detail')).toBeVisible();
    const panes = demo.locator('.overflow-y-auto, .overflow-auto, [data-slot="scroll-area-viewport"]');
    expect(await panes.evaluateAll((elements) => elements.every((element) => getComputedStyle(element).overflowY === 'clip'))).toBe(true);
    const bounds = await iframe.boundingBox();
    const before = await page.evaluate(() => scrollY);
    await page.mouse.move(bounds!.x + bounds!.width * .8, Math.max(100, Math.min(800, bounds!.y + bounds!.height / 2)));
    await page.mouse.wheel(0, 500);
    await expect.poll(() => page.evaluate(() => scrollY)).toBeGreaterThan(before + 100);
    expect(await panes.evaluateAll((elements) => elements.every((element) => element.scrollTop === 0))).toBe(true);
  });
}
