import { expect, test } from '../../fixtures';
import { mockClipboard, openEntry, unlock } from '../../ui-helpers';

test('env editor: import .env, save, copy as .env and Run…', async ({ page }) => {
  await unlock(page);
  await openEntry(page, 'acme', 'AWS prod');
  const editor = page.getByTestId('env-editor');
  await expect(editor.getByTestId('env-row')).toHaveCount(4);

  await page.getByRole('button', { name: 'Edit' }).click();
  await editor.getByRole('button', { name: 'Import .env' }).click();
  await page
    .getByLabel('.env contents')
    .fill('FEATURE_FLAG=on\nSTRIPE_SECRET_KEY=sk_test_123\nAWS_REGION=us-east-1\n');
  await page.getByRole('dialog').getByRole('button', { name: 'Import' }).click();
  await expect(editor.getByTestId('env-row')).toHaveCount(6);
  await page.getByRole('button', { name: 'Save' }).click();
  await expect(page.getByText('Saved')).toBeVisible();

  await editor.getByRole('button', { name: 'Reveal AWS_REGION' }).click();
  await expect(editor.getByRole('textbox', { name: 'AWS_REGION' })).toHaveValue('us-east-1');
  await expect(editor.getByText('FEATURE_FLAG')).toBeVisible();
  await editor.getByRole('button', { name: 'Reveal STRIPE_SECRET_KEY' }).click();
  await expect(editor.getByRole('textbox', { name: 'STRIPE_SECRET_KEY' })).toHaveValue(
    'sk_test_123'
  );

  // Bulk copy goes through the agent (env.copy): the formatted env lands on
  // the Rust fixture clipboard with auto-clear, never in the webview.
  await page.evaluate(() => navigator.clipboard.writeText('sentinel'));
  await editor.getByRole('button', { name: 'Copy as…' }).click();
  await page.getByRole('menuitem', { name: 'Copy as .env' }).click();
  await expect(page.getByText('Copied as .env')).toBeVisible();
  await expect(page.getByText(/Clipboard clears in \d+s/).first()).toBeVisible();
  await expect.poll(async () => await mockClipboard(page)).toContain('AWS_REGION=');
  const copied = await mockClipboard(page);
  expect(copied).toContain('AWS_REGION=us-east-1');
  expect(copied).toContain('STRIPE_SECRET_KEY=sk_test_123');
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe('sentinel');

  await editor.getByRole('button', { name: 'Copy as…' }).click();
  await page.getByRole('menuitem', { name: 'Copy as exports' }).click();
  await expect.poll(async () => await mockClipboard(page)).toContain("export FEATURE_FLAG='on'");

  await editor.getByRole('button', { name: 'Run…' }).click();
  await page.getByLabel('Command').fill('node server.js');
  await expect(page.getByTestId('run-command')).toHaveText(
    "latch env acme 'AWS prod' -- node server.js"
  );
});
