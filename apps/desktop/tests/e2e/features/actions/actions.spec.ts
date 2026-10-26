import { expect, test } from '../../fixtures';
import { profileLink, unlock } from '../../ui-helpers';

test('saved play actions persist edits and fail clearly until a project folder is configured', async ({
  page
}) => {
  await unlock(page);
  await profileLink(page, 'acme').click();
  await page.getByRole('button', { name: 'Actions', exact: true }).click();
  const dialog = page.getByRole('dialog');
  await expect(dialog.getByText('No actions yet')).toBeVisible();
  await dialog.getByRole('button', { name: 'New action' }).click();
  await dialog.getByLabel('Name', { exact: true }).fill('Start development');
  await dialog.getByLabel('Command', { exact: true }).fill('bun run dev');
  await dialog.getByRole('checkbox', { name: 'AWS prod' }).check();
  await dialog.getByLabel('Variable names').fill('AWS_REGION');
  await dialog.getByRole('button', { name: 'Save action' }).click();
  await expect(dialog.getByRole('button', { name: 'Play Start development' })).toBeVisible();
  await dialog.getByRole('button', { name: 'Play Start development' }).click();
  await expect(page.getByText(/set this profile's local project directory/)).toBeVisible();
  await dialog.getByRole('button', { name: 'Edit Start development' }).click();
  await expect(dialog.getByLabel('Command', { exact: true })).toHaveValue('bun run dev');
  await expect(dialog.getByLabel('Variable names')).toHaveValue('AWS_REGION');
  await dialog.getByLabel('Name', { exact: true }).fill('Develop locally');
  await dialog.getByRole('button', { name: 'Save action' }).click();
  await expect(dialog.getByRole('button', { name: 'Play Develop locally' })).toBeVisible();
  await page.keyboard.press('Escape');
  await page.getByRole('button', { name: 'Actions', exact: true }).click();
  await expect(dialog.getByRole('button', { name: 'Play Develop locally' })).toBeVisible();
  await dialog.getByRole('button', { name: 'Delete Develop locally' }).click();
  await page.getByRole('button', { name: 'Delete action', exact: true }).click();
  await expect(dialog.getByText('No actions yet')).toBeVisible();
});

test('Actions keep the profile title and editor usable at the native minimum', async ({ page }) => {
  await page.setViewportSize({ width: 820, height: 540 });
  await unlock(page);
  await profileLink(page, 'acme').click();
  await expect(page.getByRole('heading', { name: 'acme', exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Actions', exact: true }).click();
  await page.getByRole('button', { name: 'New action', exact: true }).click();
  await expect(page.getByLabel('Name', { exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Save action' })).toBeVisible();
});
