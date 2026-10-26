import { expect, test } from '../../fixtures';
import { fixture, openEntry, unlock } from '../../ui-helpers';

test('add and remove an age recipient persists across lock and unlock', async ({ page }) => {
  await unlock(page);
  const metadata = (await fixture(page)) as Awaited<ReturnType<typeof fixture>> & {
    recipientPublicKey: string;
  };
  // The Rust fixture owns the key, and the bridge allows native age recipients
  // only. SSH/plugin recipients and arbitrary executable identities stay denied.
  expect(metadata.recipientPublicKey).toMatch(/^age1/);
  await page.getByRole('link', { name: 'Recipients', exact: true }).click();
  const rows = page.getByTestId('recipient-row');
  await expect(rows.filter({ hasText: 'Passphrase' })).toHaveCount(1);
  const before = await rows.count();
  await page.getByRole('button', { name: 'Add recipient', exact: true }).click();
  const add = page.getByRole('dialog', { name: 'Add recipient', exact: true });
  await add.getByRole('button', { name: 'Add recipient', exact: true }).click();
  await expect(add.getByLabel('Label', { exact: true })).toHaveAttribute('aria-invalid', 'true');
  await expect(add.getByLabel('Public key', { exact: true })).toHaveAttribute(
    'aria-invalid',
    'true'
  );
  await add.getByLabel('Label', { exact: true }).fill('teammate-device');
  await add.getByLabel('Public key', { exact: true }).fill(metadata.recipientPublicKey);
  await add.getByRole('button', { name: 'Add recipient', exact: true }).click();
  await expect(add).toBeHidden();
  await expect(rows).toHaveCount(before + 1);
  await expect(rows.filter({ hasText: 'teammate-device' })).toContainText(
    metadata.recipientPublicKey
  );

  await page.getByRole('button', { name: 'Lock vault', exact: true }).click();
  await page.getByLabel('Passphrase', { exact: true }).fill(metadata.passphrase);
  await page.getByRole('button', { name: 'Unlock', exact: true }).click();
  await page.getByRole('link', { name: 'Recipients', exact: true }).click();
  await expect(rows).toHaveCount(before + 1);
  await expect(rows.filter({ hasText: 'teammate-device' })).toBeVisible();

  await page.getByRole('button', { name: 'Remove teammate-device', exact: true }).click();
  const confirmation = page.getByRole('alertdialog');
  await expect(confirmation).toContainText('History stays readable to them');
  await confirmation.getByRole('button', { name: 'Cancel', exact: true }).click();
  await expect(rows).toHaveCount(before + 1);
  await page.getByRole('button', { name: 'Remove teammate-device', exact: true }).click();
  await confirmation.getByRole('button', { name: 'Remove recipient', exact: true }).click();
  await expect(confirmation).toBeHidden();
  await expect(rows).toHaveCount(before);

  await page.getByRole('button', { name: 'Lock vault', exact: true }).click();
  await page.getByLabel('Passphrase', { exact: true }).fill(metadata.passphrase);
  await page.getByRole('button', { name: 'Unlock', exact: true }).click();
  await page.getByRole('link', { name: 'Recipients', exact: true }).click();
  await expect(rows).toHaveCount(before);
  await expect(rows.filter({ hasText: 'teammate-device' })).toHaveCount(0);
  // Removing the extra recipient leaves the passphrase recipient usable and
  // existing encrypted entries readable after reopening the unlocked vault.
  await openEntry(page, 'acme', 'GitHub');
  await page.getByRole('button', { name: 'Reveal Password', exact: true }).click();
  await expect(page.getByRole('textbox', { name: 'Password', exact: true })).toHaveValue(
    'hunter2-acme-Gh!'
  );
});
