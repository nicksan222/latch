import { expect, test } from '../../fixtures';
import { unlock } from '../../ui-helpers';

test('settings theme toggle applies the .dark class', async ({ page }) => {
  await unlock(page);
  await page.getByRole('link', { name: 'Settings', exact: true }).click();
  await expect(page.getByTestId('vault-fingerprint')).toHaveText(/^[A-Z0-9]{4}(-[A-Z0-9]{4}){7}$/);
  await expect(page.getByTestId('vault-notices')).toHaveCount(0);
  const html = page.locator('html');
  await page.getByRole('button', { name: 'Dark theme' }).click();
  await expect(html).toHaveClass(/dark/);
  await page.getByRole('button', { name: 'Light theme' }).click();
  await expect(html).not.toHaveClass(/dark/);
});

test('doctor reads the real healthy vault and import reports a missing file', async ({ page }) => {
  await unlock(page);
  await page.getByRole('link', { name: 'Settings', exact: true }).click();
  await page.getByRole('button', { name: 'Run doctor' }).click();
  await expect(page.getByTestId('doctor-report')).toBeVisible();
  await expect(page.getByTestId('doctor-report')).not.toContainText('manifest_stale');
  await page.getByRole('link', { name: 'Import / Export' }).click();
  await page.getByLabel('File or directory').fill('~/missing-bitwarden.json');
  await page.getByRole('button', { name: 'Preview' }).click();
  await expect(page.getByText('Preview failed')).toBeVisible();
});
