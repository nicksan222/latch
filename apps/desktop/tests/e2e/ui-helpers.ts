import { expect, type Page } from '@playwright/test';

/** Metadata comes from the Rust fixture after bootstrap. */
export async function fixture(page: Page) {
  return page.evaluate(async () => {
    const backend = window.__latchFixture;
    if (!backend) throw new Error('Rust fixture controller is not installed');
    await backend.ready;
    return backend.fixture;
  });
}

export async function unlock(page: Page, query = '?fast') {
  await page.goto(`/${query}`);
  await expect(page.getByLabel('Passphrase')).toBeVisible();
  await page.getByLabel('Passphrase').fill((await fixture(page)).passphrase);
  await page.getByRole('button', { name: 'Unlock' }).click();
  await expect(page.getByRole('list', { name: 'Profiles' })).toBeVisible();
}

export async function mockClipboard(page: Page): Promise<string | null> {
  return page.evaluate(async () => {
    const backend = window.__latchFixture;
    if (!backend) throw new Error('Rust fixture controller is not installed');
    return (await backend.inspect()).clipboard;
  });
}

export function profileLink(page: Page, name: string) {
  return page.getByRole('list', { name: 'Profiles' }).getByRole('link', { name, exact: true });
}

export async function openEntry(page: Page, profile: string, entry: string) {
  await profileLink(page, profile).click();
  await page.getByRole('link', { name: new RegExp(`^${entry}`) }).click();
  await expect(
    page.getByTestId('entry-detail').getByRole('heading', { name: entry })
  ).toBeVisible();
}
