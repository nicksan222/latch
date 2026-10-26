import { Theme } from '@latch/rpc/v1';
import { SidebarProvider } from '@latch/ui/sidebar';
import { cleanup, act, screen, within, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, afterEach, test, expect } from 'vite-plus/test';
import { useThemeStore } from '~/stores/theme';
import { renderWithProviders, setupFixture } from '~/test/helpers';
import { DesignSystemPage } from './design-system-page';

let backend: Awaited<ReturnType<typeof setupFixture>>;
beforeEach(async () => {
  backend = await setupFixture();
});
afterEach(async () => {
  cleanup();
  act(() => useThemeStore.getState().setTheme(Theme.SYSTEM));
  await backend.dispose();
});
function renderPage() {
  return renderWithProviders(
    <SidebarProvider>
      <DesignSystemPage />
    </SidebarProvider>
  );
}
test('names the reference and associates field guidance and validation', async () => {
  renderPage();
  expect(screen.getByRole('heading', { level: 1, name: 'Design system' })).toBeVisible();
  expect(screen.getByRole('textbox', { name: 'Profile name' })).toHaveAccessibleDescription(
    'A short, recognizable name for this demo profile.'
  );
  const invalid = screen.getByRole('textbox', { name: 'Remote URL' });
  expect(invalid).toBeInvalid();
  expect(invalid).toHaveAccessibleDescription(
    'Enter a valid Git URL. Use an SSH or HTTPS repository URL.'
  );
  expect(screen.getByLabelText('Demo password')).toHaveAttribute('type', 'password');
  await waitFor(() => expect(screen.getByRole('button', { name: 'Dark' })).toBeEnabled());
});
test('opens and dismisses the example dialog', async () => {
  const user = userEvent.setup();
  renderPage();
  await user.click(screen.getByRole('button', { name: 'Open example dialog' }));
  const dialog = await screen.findByRole('dialog', { name: 'Example dialog' });
  expect(dialog).toHaveAccessibleDescription(
    'A focused confirmation with a clear title and explicit actions. This example changes no vault data.'
  );
  await user.click(within(dialog).getByRole('button', { name: 'Done' }));
  await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
});
test('keeps disabled and pending controls inert and applies local themes', async () => {
  const user = userEvent.setup();
  renderPage();
  expect(screen.getByRole('button', { name: 'Disabled' })).toBeDisabled();
  expect(screen.getByRole('button', { name: 'Saving…' })).toBeDisabled();
  expect(screen.getByRole('button', { name: 'Saving…' })).toHaveAttribute('aria-busy', 'true');
  await user.click(screen.getByRole('button', { name: 'Disabled' }));
  await user.click(screen.getByRole('button', { name: 'Saving…' }));
  expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  const unavailable = screen.getByRole('textbox', { name: 'Unavailable field' });
  await user.type(unavailable, 'changed');
  expect(unavailable).toHaveValue('Read-only example');
  await user.click(screen.getByRole('button', { name: 'Dark' }));
  expect(document.documentElement).toHaveClass('dark');
  expect(screen.getByRole('button', { name: 'Dark' })).toHaveAttribute('aria-pressed', 'true');
  await user.click(screen.getByRole('button', { name: 'Light' }));
  expect(document.documentElement).not.toHaveClass('dark');
  await user.click(screen.getByRole('button', { name: 'System' }));
  expect(useThemeStore.getState().theme).toBe(Theme.SYSTEM);
});

test('waits for initial settings before enabling local theme controls', async () => {
  await backend.dispose();
  backend = await setupFixture({ unlocked: true, latencyMs: 150 });
  renderPage();
  const dark = screen.getByRole('button', { name: 'Dark' });
  expect(dark).toBeDisabled();
  await waitFor(() => expect(dark).toBeEnabled());
  await userEvent.setup().click(dark);
  expect(document.documentElement).toHaveClass('dark');
});
