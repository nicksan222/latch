import { expect, test } from '../../fixtures';
import { fixture } from '../../ui-helpers';

test('create a vault from scratch with a recovery kit', async ({ page }) => {
  await page.goto('/?fresh&fast');
  await expect(page.getByRole('heading', { name: 'Welcome' })).toBeVisible();
  await page.getByRole('button', { name: /Create a new vault/ }).click();

  await expect(page.getByLabel('Vault folder')).toHaveValue('~/latch-vault');
  await page.getByRole('button', { name: 'Continue' }).click();

  const strength = page.getByTestId('strength-meter');
  await page.getByLabel('Master passphrase').fill('password');
  await expect(strength).toContainText(/weak/i);
  await page.getByLabel('Master passphrase').fill((await fixture(page)).passphrase);
  await expect(strength).toContainText(/strong/i);
  await page.getByLabel('Confirm passphrase').fill(`${(await fixture(page)).passphrase}x`);
  await expect(page.getByText('Passphrases do not match')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Continue' })).toBeDisabled();
  await page.getByLabel('Confirm passphrase').fill((await fixture(page)).passphrase);
  await expect(page.getByTestId('kdf-benchmark')).toContainText('MiB');
  await page.getByRole('button', { name: 'Continue' }).click();

  await page.getByLabel('Git name').fill('Ada');
  await page.getByLabel('Git email').fill('ada@example.com');
  await page.getByLabel('Remote URL').fill('git@github.com:ada/vault.git');
  await page.getByRole('button', { name: 'Create vault' }).click();

  const kit = page.getByTestId('recovery-kit');
  await expect(kit).toContainText('AGE-SECRET-KEY-1');
  const open = page.getByRole('button', { name: 'Open Latch' });
  await expect(open).toBeDisabled();
  await page.getByRole('checkbox', { name: 'I saved my recovery kit somewhere safe' }).click();
  await open.click();

  await expect(page.getByText('Create your first profile')).toBeVisible();
});

test('external clone reports the development bridge denial', async ({ page }) => {
  await page.goto('/?fresh&fast');
  await page.getByRole('button', { name: /Clone a vault/ }).click();
  await page.getByLabel('Repository URL').fill('git@github.com:team/vault.git');
  await page.getByRole('button', { name: 'Clone vault' }).click();
  await expect(page.getByRole('alert')).toContainText(/unavailable|denied/i);
  await expect(page.getByRole('button', { name: 'Clone vault' })).toBeEnabled();
});
