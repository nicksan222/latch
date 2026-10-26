import { expect, test } from '../../fixtures';
import { mockClipboard, unlock } from '../../ui-helpers';

test('command palette: Enter copies, Shift+Enter copies TOTP, Ctrl+Enter opens', async ({
  page
}) => {
  await unlock(page);
  await page.keyboard.press('Control+k');
  const palette = page.getByTestId('command-palette');
  await expect(palette).toBeVisible();
  await palette.getByRole('combobox').fill('github');
  await expect(palette.getByTestId('palette-item')).toHaveCount(1);
  await page.keyboard.press('Enter');
  await expect(palette).toBeHidden();
  expect(await mockClipboard(page)).toBe('hunter2-acme-Gh!');

  await page.keyboard.press('Control+k');
  await palette.getByRole('combobox').fill('github');
  await expect(palette.getByTestId('palette-item')).toHaveCount(1);
  await page.keyboard.press('Shift+Enter');
  await expect.poll(async () => await mockClipboard(page)).toMatch(/^\d{6}$/);

  await page.keyboard.press('Control+k');
  await palette.getByRole('combobox').fill('postgres');
  await expect(palette.getByTestId('palette-item')).toHaveCount(1);
  await page.keyboard.press('Control+Enter');
  await expect(
    page.getByTestId('entry-detail').getByRole('heading', { name: 'Postgres prod' })
  ).toBeVisible();
});
