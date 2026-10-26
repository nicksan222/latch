import { expect, test } from '../../fixtures';
import { mockClipboard, openEntry, unlock, profileLink } from '../../ui-helpers';

test.beforeEach(async ({ page }) => {
  await unlock(page);
});

test('create a login entry with TOTP', async ({ page }) => {
  await profileLink(page, 'side-project').click();
  await page.getByRole('button', { name: 'New entry' }).click();
  await page.getByRole('menuitem', { name: 'Login' }).click();
  await page.getByRole('textbox', { name: 'Name', exact: true }).fill('Linear');
  await page.getByRole('textbox', { name: 'Username' }).fill('nick@side.dev');
  await page.getByLabel('Password', { exact: true }).fill('linear-pass-1');
  await page.getByRole('button', { name: 'Add TOTP' }).click();
  await page.getByLabel('TOTP secret').fill('JBSWY3DPEHPK3PXP');
  await page.getByRole('button', { name: 'Save' }).click();

  const detail = page.getByTestId('entry-detail');
  await expect(detail.getByRole('heading', { name: 'Linear' })).toBeVisible();
  await expect(detail.getByTestId('totp-field')).toBeVisible();
  await detail.getByRole('button', { name: 'Reveal one-time code' }).click();
  await expect(detail.getByRole('textbox', { name: 'One-time code' })).toHaveValue(/^\d{3} \d{3}$/);
  await expect(page.getByTestId('entry-row').filter({ hasText: 'Linear' })).toContainText(/\d+s/);
});

test('validation blocks saving an invalid entry', async ({ page }) => {
  await profileLink(page, 'acme').click();
  await page.getByRole('button', { name: 'New entry' }).click();
  await page.getByRole('menuitem', { name: 'API key' }).click();
  await page.getByRole('button', { name: 'Save' }).click();
  await expect(page.getByText('Name is required')).toBeVisible();
  await expect(page.getByText('Key is required')).toBeVisible();
});

test('reveal auto-hides after the configured delay', async ({ page }) => {
  await page.getByRole('link', { name: 'Settings', exact: true }).click();
  await page.getByRole('combobox', { name: 'Reveal timeout' }).click();
  await page.getByRole('option', { name: '5 seconds' }).click();
  await openEntry(page, 'acme', 'GitHub');
  const password = page.getByRole('textbox', { name: 'Password', exact: true });
  await expect(password).toHaveValue('••••••••••••');
  await page.getByRole('button', { name: 'Reveal Password' }).click();
  await expect(password).toHaveValue('hunter2-acme-Gh!');
  await expect(password).toHaveValue('••••••••••••', { timeout: 8_000 });
});

test('reveal is dropped on navigation', async ({ page }) => {
  await openEntry(page, 'acme', 'GitHub');
  await page.getByRole('button', { name: 'Reveal Password' }).click();
  await expect(page.getByRole('textbox', { name: 'Password', exact: true })).toHaveValue(
    'hunter2-acme-Gh!'
  );
  await page.getByRole('link', { name: /^Stripe/ }).click();
  await page.getByRole('link', { name: /^GitHub/ }).click();
  await expect(page.getByRole('textbox', { name: 'Password', exact: true })).toHaveValue(
    '••••••••••••'
  );
});

test('copy goes through clipboard.copy', async ({ page }) => {
  await profileLink(page, 'acme').click();
  await page.getByRole('button', { name: 'Copy password of GitHub' }).click();
  await expect(page.getByText('Copied password')).toBeVisible();
  expect(await mockClipboard(page)).toBe('hunter2-acme-Gh!');
  // The webview never received the plaintext.
  await expect(page.getByText('hunter2-acme-Gh!')).toHaveCount(0);
});

test('edit an entry fetches revealed values and saves', async ({ page }) => {
  await openEntry(page, 'acme', 'Stripe');
  await page.getByRole('button', { name: 'Edit' }).click();
  await expect(page.getByLabel('Key', { exact: true })).toHaveValue(
    'sk_live_51Hacme0000000000000000'
  );
  await page.getByLabel('Endpoint').fill('https://api.stripe.com/v3');
  await page.getByRole('button', { name: 'Save' }).click();
  await expect(page.getByText('https://api.stripe.com/v3').first()).toBeVisible();
});

test('clearing an optional API key secret updates the entry view', async ({ page }) => {
  await openEntry(page, 'acme', 'Stripe');
  const detail = page.getByTestId('entry-detail');
  await expect(detail.getByRole('button', { name: 'Reveal Secret' })).toBeVisible();
  await page.getByRole('button', { name: 'Edit' }).click();
  await expect(page.getByLabel('Secret (optional)')).toHaveValue('whsec_acme_000000000000');
  await page.getByLabel('Secret (optional)').fill('');
  await page.getByRole('button', { name: 'Save' }).click();
  await expect(page.getByText('Saved')).toBeVisible();
  // The cleared secret has no reveal action, while the key remains available.
  await expect(detail.getByRole('button', { name: 'Reveal Secret' })).toHaveCount(0);
  await expect(detail.getByRole('button', { name: 'Reveal Key' })).toBeVisible();
});
