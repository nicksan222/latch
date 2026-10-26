import { expect, test } from '../../fixtures';
import { openEntry, unlock } from '../../ui-helpers';

test.beforeEach(async ({ page }) => {
  await unlock(page);
});

test('history restore', async ({ page }) => {
  await openEntry(page, 'acme', 'GitHub');
  await page.getByRole('tab', { name: 'History' }).click();
  await expect(page.getByTestId('history-item')).toHaveCount(2);
  await page.getByRole('button', { name: 'Restore' }).click();
  await page.getByRole('button', { name: 'Restore this version' }).click();
  await expect(page.getByText('Version restored')).toBeVisible();
  await expect(page.getByTestId('history-item')).toHaveCount(3);
  await page.getByRole('tab', { name: 'Details' }).click();
  await page.getByRole('button', { name: 'Reveal Password' }).click();
  await expect(page.getByRole('textbox', { name: 'Password', exact: true })).toHaveValue(
    'old-password-2025'
  );
});

test('delete an entry and restore it from trash', async ({ page }) => {
  await openEntry(page, 'acme', 'Runbook');
  await page.getByRole('button', { name: 'Delete entry' }).click();
  await page.getByRole('alertdialog').getByRole('button', { name: 'Delete entry' }).click();
  await expect(page.getByRole('link', { name: /^Runbook/ })).toHaveCount(0);
  await page.getByRole('link', { name: 'Trash' }).click();
  await page
    .getByTestId('trash-row')
    .filter({ hasText: 'Runbook' })
    .getByRole('button', { name: 'Restore' })
    .click();
  await expect(page.getByText('Restored Runbook')).toBeVisible();
});
