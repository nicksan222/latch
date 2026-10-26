import { expect, test } from '../../fixtures';
import { fixture, unlock, profileLink } from '../../ui-helpers';

test('wrong passphrase shows an error, right one unlocks', async ({ page }) => {
  await page.goto('/?fast');
  await expect(page.getByRole('heading', { name: 'Latch is locked' })).toBeVisible();
  await expect(page.getByText((await fixture(page)).vaults.personal!).first()).toBeVisible();
  await page.getByLabel('Passphrase').fill('not the passphrase');
  await page.getByRole('button', { name: 'Unlock' }).click();
  await expect(
    page.getByRole('alert').filter({ hasText: /credentials|passphrase/i })
  ).toBeVisible();
  await expect(page.getByLabel('Passphrase')).toHaveAttribute('aria-invalid', 'true');
  await expect(page.getByLabel('Passphrase')).toHaveValue('');

  await page.getByLabel('Passphrase').fill((await fixture(page)).passphrase);
  await page.keyboard.press('Enter');
  await expect(profileLink(page, 'acme')).toBeVisible();
  await expect(profileLink(page, 'personal')).toBeVisible();
  await expect(profileLink(page, 'side-project')).toBeVisible();
});

test('external identity-file unlock is denied by the development bridge', async ({ page }) => {
  await page.goto('/?fast');
  await page.getByRole('button', { name: 'Use an identity file instead' }).click();
  await page.getByLabel('Identity file').fill('~/bad-identity.txt');
  await page.getByRole('button', { name: 'Unlock' }).click();
  await expect(page.getByRole('alert')).toContainText(/unavailable|denied/i);
  await expect(page.getByRole('heading', { name: 'Latch is locked' })).toBeVisible();
});

test('lock button returns to the lock screen and drops data', async ({ page }) => {
  await unlock(page);
  await page.getByRole('button', { name: 'Lock vault' }).click();
  await expect(page.getByRole('heading', { name: 'Latch is locked' })).toBeVisible();
  await expect(profileLink(page, 'acme')).toHaveCount(0);
});

test('switch to another vault from the lock screen', async ({ page }) => {
  await page.goto('/?fast');
  await expect(page.getByRole('combobox', { name: 'Switch vault' })).toBeVisible();
  const employerPath = (await fixture(page)).vaults.employer!;
  await page.getByRole('combobox', { name: 'Switch vault' }).click();
  await page.getByRole('option', { name: employerPath }).click();
  await expect(page.getByText(employerPath).first()).toBeVisible();
  await page.getByLabel('Passphrase').fill((await fixture(page)).passphrase);
  await page.getByRole('button', { name: 'Unlock' }).click();
  await expect(profileLink(page, 'employer')).toBeVisible();
});
