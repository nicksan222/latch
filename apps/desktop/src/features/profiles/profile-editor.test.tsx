import { SidebarProvider } from '@latch/ui/sidebar';
import {
  createMemoryHistory,
  createRootRoute,
  createRouter,
  RouterProvider
} from '@tanstack/react-router';
import { cleanup, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it } from 'vite-plus/test';
import { setTransport } from '~/lib/api/client';
import type { FixtureClient } from '~/lib/api/fixture';
import { renderWithProviders, setupFixture } from '~/test/helpers';
import { ProfileEditor } from './profile-editor';

let backend: FixtureClient;
beforeEach(async () => {
  backend = await setupFixture();
});
afterEach(async () => {
  cleanup();
  await backend.dispose();
  setTransport(null);
});

function renderEditor() {
  const routeTree = createRootRoute({
    component: () => (
      <SidebarProvider>
        <ProfileEditor mode="create" />
      </SidebarProvider>
    )
  });
  const router = createRouter({
    routeTree,
    history: createMemoryHistory({ initialEntries: ['/'] })
  });
  renderWithProviders(<RouterProvider router={router} />);
}

describe('ProfileEditor row identity', () => {
  it('does not expose host browser discovery or engine and binary selection', async () => {
    renderEditor();

    expect(await screen.findByRole('heading', { name: 'Browser' })).toBeInTheDocument();
    expect(screen.queryByRole('combobox', { name: 'Browser engine' })).not.toBeInTheDocument();
    expect(screen.queryByRole('textbox', { name: 'Browser binary' })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Detect browsers' })).not.toBeInTheDocument();
    expect(screen.getByText(/Latch-managed Firefox/i)).toBeInTheDocument();
  });

  it('retains a start page control through editing and deletion of an earlier empty page', async () => {
    const user = userEvent.setup();
    renderEditor();
    await user.click(await screen.findByRole('button', { name: 'Add start page' }));
    await user.click(screen.getByRole('button', { name: 'Add start page' }));
    const second = screen.getByLabelText('Start page 2');
    await user.type(second, 'https://example.com');
    expect(screen.getByLabelText('Start page 2')).toBe(second);
    await user.click(screen.getByRole('button', { name: 'Remove Start page 1' }));
    expect(screen.getByLabelText('Start page 1')).toBe(second);
    expect(second).toHaveValue('https://example.com');
  });

  it('retains bookmark title and URL controls after removing the previous bookmark', async () => {
    const user = userEvent.setup();
    renderEditor();
    await user.click(await screen.findByRole('button', { name: 'Add bookmark' }));
    await user.click(screen.getByRole('button', { name: 'Add bookmark' }));
    const title = screen.getByLabelText('Bookmark 2 title');
    const url = screen.getByLabelText('Bookmark 2 URL');
    await user.type(title, 'Docs');
    await user.type(url, 'https://example.com/docs');
    await user.click(screen.getByRole('button', { name: 'Remove bookmark 1' }));
    expect(screen.getByLabelText('Bookmark 1 title')).toBe(title);
    expect(screen.getByLabelText('Bookmark 1 URL')).toBe(url);
    expect(title).toHaveValue('Docs');
    expect(url).toHaveValue('https://example.com/docs');
  });
});
