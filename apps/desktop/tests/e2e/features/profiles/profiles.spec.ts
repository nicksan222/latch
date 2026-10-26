import { expect, test } from '../../fixtures';
import { profileLink, unlock } from '../../ui-helpers';

test.beforeEach(async ({ page }) => {
  await unlock(page);
});

test('create a profile', async ({ page }) => {
  await page.getByRole('link', { name: 'New profile' }).click();
  await page.getByRole('textbox', { name: 'Name', exact: true }).fill('client-x');
  await page.getByRole('radio', { name: '#7c3aed' }).click();
  await page.getByRole('button', { name: 'Create profile' }).click();
  await expect(profileLink(page, 'client-x')).toBeVisible();
  await expect(page.locator('header').getByRole('heading', { name: 'client-x' })).toBeVisible();
  await expect(page.getByText('No entries yet')).toBeVisible();
});
