import { expect, test } from '../../fixtures';
import { profileLink } from '../../ui-helpers';

for (const [path, title] of [
  ['settings', 'Settings'],
  ['sync', 'Sync'],
  ['recipients', 'Recipients'],
  ['import-export', 'Import / Export'],
  ['trash', 'Trash'],
  ['profiles/new', 'New profile']
]) {
  test(`${title} has a compact accessible page title at the native minimum`, async ({ page }) => {
    await page.setViewportSize({ width: 820, height: 540 });
    await page.goto(`/?unlocked&fast#/${path}`);
    await expect(page.getByRole('heading', { level: 1, name: title, exact: true })).toBeVisible();
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
      true
    );
    await expect(page.getByRole('button', { name: 'Lock vault', exact: true })).toBeVisible();
  });
}

test('development reference exposes the shared components in both themes', async ({ page }) => {
  await page.goto('/?unlocked&fast#/design-system');
  await expect(page.getByRole('heading', { name: 'Design system', level: 1 })).toBeVisible();
  await expect(page.getByRole('region', { name: 'Typography & surfaces' })).toBeVisible();
  await page.getByRole('button', { name: 'Dark', exact: true }).click();
  await expect(page.locator('html')).toHaveClass(/dark/);
  await page.getByRole('button', { name: 'Light', exact: true }).click();
  await expect(page.locator('html')).not.toHaveClass(/dark/);
});

test('entry detail keeps its title and edit action usable in a compact split pane', async ({
  page
}) => {
  await page.setViewportSize({ width: 820, height: 540 });
  await page.goto('/?unlocked&fast');
  await profileLink(page, 'acme').click();
  await page.getByRole('link', { name: /^GitHub/ }).click();
  const detail = page.getByTestId('entry-detail');
  const title = detail.getByRole('heading', { name: 'GitHub', exact: true });
  await expect(title).toBeVisible();
  expect((await title.boundingBox())?.width).toBeGreaterThan(80);
  await expect(detail.getByRole('button', { name: 'Edit', exact: true })).toBeInViewport();
});

test('profile URL errors stay attached to their original row when earlier rows are blank', async ({
  page
}) => {
  await page.goto('/?unlocked&fast#/profiles/new');
  await page.getByRole('textbox', { name: 'Name', exact: true }).fill('URL validation');
  await page.getByRole('button', { name: 'Add start page', exact: true }).click();
  await page.getByRole('button', { name: 'Add start page', exact: true }).click();
  await page.getByRole('textbox', { name: 'Start page 2', exact: true }).fill('invalid-url');
  await page.getByRole('button', { name: 'Create profile', exact: true }).click();
  await expect(page.getByRole('textbox', { name: 'Start page 2', exact: true })).toHaveAttribute(
    'aria-invalid',
    'true'
  );
  await expect(
    page.getByRole('textbox', { name: 'Start page 1', exact: true })
  ).not.toHaveAttribute('aria-invalid', 'true');
});

test('database fields stay in one column inside a narrow entry pane', async ({ page }) => {
  await page.setViewportSize({ width: 820, height: 540 });
  await page.goto('/?unlocked&fast');
  await profileLink(page, 'acme').click();
  await page.getByRole('link', { name: /^Postgres prod/ }).click();
  await page.getByRole('button', { name: 'Edit', exact: true }).click();
  const columns = await page.getByLabel('Options', { exact: true }).evaluate((input) => {
    const grid = input.closest('.grid');
    return grid ? getComputedStyle(grid).gridTemplateColumns.split(' ').length : 0;
  });
  expect(columns).toBe(1);
});
