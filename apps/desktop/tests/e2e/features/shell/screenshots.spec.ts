import { mkdirSync } from 'node:fs';
import { join } from 'node:path';
import { expect, test } from '../../fixtures';
import { unlock, profileLink } from '../../ui-helpers';

const screenshots = join(import.meta.dirname, '../../../../../../.dev/test-results/screenshots');

test('screenshots of the main screen (light + dark)', async ({ page }) => {
  mkdirSync(screenshots, { recursive: true });
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await unlock(page);
  await profileLink(page, 'acme').click();
  await page.getByRole('link', { name: /^GitHub/ }).click();
  await expect(page.getByTestId('entry-detail')).toBeVisible();
  await page.mouse.move(0, 0);
  await page.screenshot({ path: join(screenshots, 'main-light.png') });
  await page.evaluate(() => {
    document.documentElement.classList.add('dark');
    document.documentElement.style.colorScheme = 'dark';
  });
  await page.waitForTimeout(600);
  await page.screenshot({ path: join(screenshots, 'main-dark.png') });
  await page.getByRole('link', { name: 'Sync' }).click();
  await expect(page.getByTestId('sync-stats')).toBeVisible();
  await page.waitForTimeout(300);
  await page.screenshot({ path: join(screenshots, 'sync-dark.png') });
});
