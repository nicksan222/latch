import { expect, test } from '../../fixtures';
import { fixture, openEntry, profileLink, unlock } from '../../ui-helpers';

test('encrypted export reimports into another profile, rejects a wrong passphrase and skips duplicates', async ({
  page
}) => {
  // age archive encryption/decryption uses its real KDF, including failed decrypts.
  test.setTimeout(120_000);
  await unlock(page);
  const sourceEntries = Object.keys((await fixture(page)).entries).filter((name) =>
    name.startsWith('acme/')
  );
  const archive = 'roundtrip.latch.age';
  const passphrase = 'fictional archive passphrase';

  await page.getByRole('link', { name: 'Import / Export' }).click();
  await page.getByRole('tab', { name: 'Export', exact: true }).click();
  await page.getByRole('combobox', { name: 'Profiles', exact: true }).click();
  await page.getByRole('option', { name: 'acme', exact: true }).click();
  // A virtual label resolves to the same private arena file on export and import.
  // This never reads or writes an arbitrary host destination.
  await page.getByLabel('Destination', { exact: true }).fill(archive);
  await page.getByLabel('Archive passphrase', { exact: true }).fill(passphrase);
  await page.getByRole('button', { name: 'Export', exact: true }).click();
  await expect(
    page.getByText(`Exported ${sourceEntries.length} entries`, { exact: true })
  ).toBeVisible({
    timeout: 30_000
  });

  await page.getByRole('link', { name: 'New profile', exact: true }).click();
  await page.getByRole('textbox', { name: 'Name', exact: true }).fill('archive-restored');
  await page.getByRole('button', { name: 'Create profile' }).click();
  await expect(profileLink(page, 'archive-restored')).toBeVisible();

  await page.getByRole('link', { name: 'Import / Export' }).click();
  await page.getByRole('combobox', { name: 'Format', exact: true }).click();
  await page.getByRole('option', { name: 'Latch encrypted archive', exact: true }).click();
  await page.getByLabel('File or directory', { exact: true }).fill(archive);
  await page.getByRole('combobox', { name: 'Target profile', exact: true }).click();
  await page.getByRole('option', { name: 'archive-restored', exact: true }).click();
  await page.getByLabel('Archive passphrase', { exact: true }).fill('wrong archive passphrase');
  await page.getByRole('button', { name: 'Preview', exact: true }).click();
  await expect(page.getByText('Preview failed', { exact: true })).toBeVisible({ timeout: 30_000 });
  await expect(page.getByTestId('import-preview')).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Import', exact: true })).toBeDisabled();

  await page.getByLabel('Archive passphrase', { exact: true }).fill(passphrase);
  await page.getByRole('button', { name: 'Preview', exact: true }).click();
  const preview = page.getByTestId('import-preview');
  await expect(preview.getByRole('row')).toHaveCount(sourceEntries.length + 1, { timeout: 30_000 });
  await expect(preview).toContainText('GitHub');
  await expect(preview).toContainText('archive-restored');
  // Preview contains metadata only, even after explicitly supplying the archive key.
  await expect(preview).not.toContainText('hunter2-acme-Gh!');
  await page.getByRole('button', { name: `Import ${sourceEntries.length}`, exact: true }).click();
  await expect(
    page.getByText(`Imported ${sourceEntries.length} entries`, { exact: true })
  ).toBeVisible({
    timeout: 30_000
  });

  await page.getByRole('button', { name: 'Preview', exact: true }).click();
  await expect(preview.getByText('skip · duplicate', { exact: true })).toHaveCount(
    sourceEntries.length,
    { timeout: 30_000 }
  );
  await page.getByRole('button', { name: 'Import 0', exact: true }).click();
  await expect(page.getByText('Imported 0 entries', { exact: true })).toBeVisible({
    timeout: 30_000
  });
  await expect(
    page.getByText(`${sourceEntries.length} skipped · 0 profile(s) created`)
  ).toBeVisible();

  await openEntry(page, 'archive-restored', 'GitHub');
  await page.getByRole('button', { name: 'Reveal Password', exact: true }).click();
  await expect(page.getByRole('textbox', { name: 'Password', exact: true })).toHaveValue(
    'hunter2-acme-Gh!'
  );
  await expect(page.getByRole('link', { name: /^GitHub/ })).toHaveCount(1);
});

test('plaintext export requires both confirmations and Cancel sends no export request', async ({
  page
}) => {
  let exportRequests = 0;
  page.on('request', (request) => {
    if (new URL(request.url()).pathname !== '/__latch/rpc' || request.method() !== 'POST') return;
    const body = request.postDataJSON() as { request?: { method?: string } };
    if (body.request?.method === 'export.run') exportRequests += 1;
  });
  await unlock(page);
  await page.getByRole('link', { name: 'Import / Export' }).click();
  await page.getByRole('tab', { name: 'Export', exact: true }).click();
  await page.getByRole('combobox', { name: 'Format', exact: true }).click();
  await page.getByRole('option', { name: 'Plaintext JSON', exact: true }).click();
  // A valid arena label ensures an accidental export would actually write.
  await page.getByLabel('Destination', { exact: true }).fill('cancelled-export.json');
  const exportButton = page.getByRole('button', { name: 'Export plaintext…', exact: true });
  await expect(page.getByTestId('plaintext-warning')).toBeVisible();
  await expect(exportButton).toBeDisabled();
  await page.getByRole('checkbox', { name: 'I understand the risk' }).check();
  await exportButton.click();
  const confirmation = page.getByRole('alertdialog');
  await expect(
    confirmation.getByRole('heading', { name: 'Export secrets in plaintext?' })
  ).toBeVisible();
  await confirmation.getByRole('button', { name: 'Cancel', exact: true }).click();
  await expect(confirmation).toBeHidden();
  expect(
    await page.evaluate(async () => (await window.__latchFixture!.inspect()).lastExport)
  ).toBeNull();
  expect(exportRequests, 'Opening and cancelling confirmation must never call export.run').toBe(0);
});

test('plaintext export reports forbidden traversal without mutation', async ({ page }) => {
  await unlock(page);
  await page.getByRole('link', { name: 'Import / Export' }).click();
  await page.getByRole('tab', { name: 'Export', exact: true }).click();
  await page.getByRole('combobox', { name: 'Format', exact: true }).click();
  await page.getByRole('option', { name: 'Plaintext JSON', exact: true }).click();
  await page.getByLabel('Destination', { exact: true }).fill('../outside-arena.json');
  await page.getByRole('checkbox', { name: 'I understand the risk' }).check();
  await page.getByRole('button', { name: 'Export plaintext…', exact: true }).click();
  const confirmation = page.getByRole('alertdialog');
  await confirmation.getByRole('button', { name: 'Write plaintext file', exact: true }).click();
  await expect(page.getByText('Export failed', { exact: true })).toBeVisible();
  // The bridge denies parent traversal instead of touching external paths. This
  // checks that boundary, not native filesystem picker or OS permissions.
  expect(
    await page.evaluate(async () => (await window.__latchFixture!.inspect()).lastExport)
  ).toBeNull();
  await expect(confirmation).toBeVisible();
  await confirmation.getByRole('button', { name: 'Cancel', exact: true }).click();
  await openEntry(page, 'acme', 'GitHub');
  await page.getByRole('button', { name: 'Reveal Password', exact: true }).click();
  await expect(page.getByRole('textbox', { name: 'Password', exact: true })).toHaveValue(
    'hunter2-acme-Gh!'
  );
});
