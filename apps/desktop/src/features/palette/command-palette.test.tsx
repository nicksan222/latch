import { create, type MessageInitShape } from '@bufbuild/protobuf';
import { EntryType, EntrySummarySchema, LatchAgent, type EntrySummary } from '@latch/rpc/v1';
import {
  createMemoryHistory,
  createRootRoute,
  createRouter,
  RouterProvider
} from '@tanstack/react-router';
import { cleanup, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';
import { setTransport } from '~/lib/api/client';
import type { FixtureClient } from '~/lib/api/fixture';
import { renderWithProviders, setupFixture } from '~/test/helpers';
import { PaletteBody } from './command-palette';
import { paletteAction } from './palette-logic';

const summary = (over: MessageInitShape<typeof EntrySummarySchema>): EntrySummary =>
  create(EntrySummarySchema, {
    id: 'e',
    profileId: 'p',
    type: EntryType.LOGIN,
    name: 'GitHub',
    tags: [],
    favorite: false,
    subtitle: undefined,
    hasTotp: true,
    updatedAt: '',
    ...over
  });

describe('paletteAction', () => {
  it('maps Enter / Shift+Enter / Ctrl+Enter per SPEC §9.3', () => {
    expect(paletteAction({ key: 'Enter' }, summary({}))).toEqual({
      kind: 'copy',
      field: 'password',
      label: 'password'
    });
    expect(paletteAction({ key: 'Enter', shiftKey: true }, summary({}))).toEqual({
      kind: 'copy',
      field: 'totp',
      label: 'one-time code'
    });
    expect(paletteAction({ key: 'Enter', ctrlKey: true }, summary({}))).toEqual({ kind: 'open' });
    expect(paletteAction({ key: 'Enter', metaKey: true }, summary({}))).toEqual({ kind: 'open' });
    expect(paletteAction({ key: 'a' }, summary({}))).toBeNull();
  });
  it('copies the primary secret per type and opens entries without one', () => {
    expect(paletteAction({ key: 'Enter' }, summary({ type: EntryType.API_KEY }))).toMatchObject({
      field: 'key'
    });
    expect(paletteAction({ key: 'Enter' }, summary({ type: EntryType.DATABASE }))).toMatchObject({
      field: 'password'
    });
    expect(paletteAction({ key: 'Enter' }, summary({ type: EntryType.ENV }))).toEqual({
      kind: 'open'
    });
    expect(
      paletteAction({ key: 'Enter', shiftKey: true }, summary({ hasTotp: false }))
    ).toMatchObject({ kind: 'none' });
  });
});

describe('PaletteBody', () => {
  let backend: FixtureClient;
  const onClose = vi.fn();
  let router: { state: { location: { pathname: string } } };

  beforeEach(async () => {
    backend = await setupFixture();
    onClose.mockReset();
    const root = createRootRoute({ component: () => <PaletteBody onClose={onClose} /> });
    router = createRouter({
      routeTree: root,
      history: createMemoryHistory({ initialEntries: ['/'] })
    }) as unknown as typeof router;
  });
  afterEach(async () => {
    cleanup();
    await backend.dispose();
    setTransport(null);
  });

  const renderPalette = async () => {
    renderWithProviders(<RouterProvider router={router as never} />);
    const input = await screen.findByRole('combobox', { name: 'Search entries' });
    await screen.findAllByTestId('palette-item');
    return input;
  };

  it('searches across profiles', async () => {
    const user = userEvent.setup();
    const input = await renderPalette();
    expect(screen.getAllByTestId('palette-item').length).toBeGreaterThan(10);
    await user.type(input, 'google');
    await waitFor(() => expect(screen.getAllByTestId('palette-item')).toHaveLength(1));
    expect(screen.getByText('Google')).toBeInTheDocument();
  });

  it('Enter copies the password via clipboard.copy', async () => {
    const user = userEvent.setup();
    const input = await renderPalette();
    await user.type(input, 'github');
    await waitFor(() => expect(screen.getAllByTestId('palette-item')).toHaveLength(1));
    await user.keyboard('{Enter}');
    await waitFor(() => expect(backend.clipboard).toBe('hunter2-acme-Gh!'));
    expect(onClose).toHaveBeenCalled();
    expect(backend.calls.map((c) => c.method)).not.toContain(LatchAgent.method.entriesReveal.name);
  });

  it('Shift+Enter copies the TOTP code', async () => {
    const user = userEvent.setup();
    const input = await renderPalette();
    await user.type(input, 'github');
    await waitFor(() => expect(screen.getAllByTestId('palette-item')).toHaveLength(1));
    await user.keyboard('{Shift>}{Enter}{/Shift}');
    await waitFor(() => expect(backend.clipboard).toMatch(/^\d{6}$/));
    expect(backend.clipboard).toMatch(/^\d{6}$/);
  });

  it('Ctrl+Enter opens the entry', async () => {
    const user = userEvent.setup();
    const input = await renderPalette();
    await user.type(input, 'stripe');
    await waitFor(() => expect(screen.getAllByTestId('palette-item')).toHaveLength(1));
    await user.keyboard('{Control>}{Enter}{/Control}');
    await waitFor(() =>
      expect(router.state.location.pathname).toBe(
        `/p/${backend.fixture.profiles.acme}/e/${backend.fixture.entries['acme/Stripe']}`
      )
    );
    expect(backend.clipboard).toBeNull();
  });
});
