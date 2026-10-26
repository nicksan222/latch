import { expect, test } from '../../fixtures';
import { openEntry, unlock } from '../../ui-helpers';

test('sync renders real local commits and explains the missing remote', async ({ page }) => {
  await unlock(page);
  await page.getByRole('link', { name: 'Sync', exact: true }).click();
  await expect(page.getByTestId('sync-stats')).toBeVisible();
  await expect(page.getByText('No remote: the vault is only on this machine.')).toBeVisible();
  await expect(page.getByRole('list', { name: 'Recent commits' })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Sync now' })).toHaveCount(0);
  await page.getByLabel('Remote URL').fill('git@example.invalid:team/vault.git');
  await page.getByRole('button', { name: 'Save', exact: true }).click();
  await expect(page.getByText('Could not set remote')).toBeVisible();
});

test('sync pulls a change from the session-local peer repository', async ({ page }) => {
  await unlock(page);
  // This fixture-only operation creates the bare remote and peer clone inside
  // the session arena. Product sync still runs through the real UI and agent.
  await page.evaluate(async () => {
    await window.__latchFixture!.syncScenario('remote_change');
  });
  await page.getByRole('link', { name: 'Sync', exact: true }).click();
  await page.getByRole('button', { name: 'Sync now', exact: true }).click();
  await expect(page.getByText('Sync complete', { exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Resolve…', exact: true })).toHaveCount(0);
  await openEntry(page, 'acme', 'GitHub');
  await page.getByRole('button', { name: 'Reveal Password', exact: true }).click();
  await expect(page.getByRole('textbox', { name: 'Password', exact: true })).toHaveValue(
    'fixture-remote-password'
  );
});

test('sync resolves real encrypted divergence using the remote version', async ({ page }) => {
  await unlock(page);
  await page.evaluate(async () => {
    await window.__latchFixture!.syncScenario('conflict');
  });
  await page.getByRole('link', { name: 'Sync', exact: true }).click();
  await page.getByRole('button', { name: 'Sync now', exact: true }).click();
  await expect(page.getByText('1 conflict to resolve', { exact: true })).toBeVisible();
  // Conflicting plaintext stays out of the page until Resolve is requested.
  await expect(page.getByText('fixture-mine-password', { exact: true })).toHaveCount(0);
  await expect(page.getByText('fixture-remote-password', { exact: true })).toHaveCount(0);
  await page.getByRole('button', { name: 'Resolve…', exact: true }).click();
  const resolver = page.getByTestId('conflict-resolver');
  await expect(
    resolver.getByRole('heading', { name: 'Resolve “GitHub”', exact: true })
  ).toBeVisible();
  await expect(resolver.getByText('fixture-mine-password', { exact: true })).toBeVisible();
  await expect(resolver.getByText('fixture-remote-password', { exact: true })).toBeVisible();
  await resolver.getByRole('button', { name: 'Keep theirs', exact: true }).click();
  await expect(resolver).toBeHidden();
  await expect(page.getByText('Resolved GitHub', { exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Resolve…', exact: true })).toHaveCount(0);
  await expect(page.getByText('fixture-mine-password', { exact: true })).toHaveCount(0);
  await expect(page.getByText('fixture-remote-password', { exact: true })).toHaveCount(0);

  await page.getByRole('button', { name: 'Sync now', exact: true }).click();
  await expect(page.getByText('Sync complete', { exact: true })).toBeVisible();
  await openEntry(page, 'acme', 'GitHub');
  const password = page.getByRole('textbox', { name: 'Password', exact: true });
  await expect(password).toHaveValue('••••••••••••');
  await page.getByRole('button', { name: 'Reveal Password', exact: true }).click();
  await expect(password).toHaveValue('fixture-remote-password');
});
