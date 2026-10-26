import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, expect, it, vi } from 'vite-plus/test';
import { setTransport } from '~/lib/api/client';
import { entryToInput } from '~/lib/entries';
import { renderWithProviders } from '~/test/helpers';
import { setupDomainResponse } from '~/test/helpers';
import type { DomainRequest, Entry } from '../../_generated/contracts';
afterEach(() => setTransport(null));
import { ConflictResolver, diffRows } from './conflict-resolver';

it('requests the Rust diff and preserves content-only difference metadata', async () => {
  const output = [
    { field: 'file', mine: 'id.pem', theirs: 'id.pem', differs: true, contentDiffers: true }
  ];
  const call = setupDomainResponse({ operation: 'diff_rows', output });
  expect(await diffRows({ entryId: 'e', mine: null, theirs: null })).toEqual(output);
  expect(call).toHaveBeenCalledWith('domain.call', {
    operation: 'diff_rows',
    input: { mine: null, theirs: null }
  });
});

it('selects fields, requests a Rust merge, and submits the returned draft', async () => {
  const user = userEvent.setup();
  const mine: Entry = {
    id: 'entry',
    profileId: 'profile',
    name: 'GitHub',
    tags: [],
    notes: 'mine',
    favorite: false,
    createdAt: '2026-01-01T00:00:00Z',
    updatedAt: '2026-01-01T00:00:00Z',
    redactedFields: [],
    payload: { type: 'secret', data: { value: 'my-fictional-value' } }
  };
  const theirs: Entry = {
    ...mine,
    notes: 'theirs',
    payload: { type: 'secret', data: { value: 'their-fictional-value' } }
  };
  const merged = { ...entryToInput(mine), payload: theirs.payload };
  const wire = vi.fn(async (method: string, params: unknown) => {
    if (method === 'sync.conflict') return { entryId: 'entry', mine, theirs };
    if (method === 'domain.call') {
      const request = params as DomainRequest;
      if (request.operation === 'diff_rows')
        return {
          operation: 'diff_rows',
          output: [
            {
              field: 'value',
              mine: 'my-fictional-value',
              theirs: 'their-fictional-value',
              differs: true,
              contentDiffers: false
            },
            { field: 'notes', mine: 'mine', theirs: 'theirs', differs: true, contentDiffers: false }
          ]
        };
      if (request.operation === 'merge_entries')
        return { operation: 'merge_entries', output: merged };
    }
    if (method === 'sync.resolve') return {};
    throw new Error(`Unexpected test RPC ${method}`);
  });
  setTransport({ kind: 'stub', call: wire, subscribe: () => () => {} });
  const onClose = vi.fn();
  const view = renderWithProviders(
    <ConflictResolver
      conflict={{ entryId: 'entry', profileId: 'profile', name: 'GitHub' }}
      onClose={onClose}
    />
  );
  await user.click(await screen.findByRole('radio', { name: 'Take their value' }));
  expect(screen.getByRole('radio', { name: 'Keep my notes' })).toBeChecked();
  await user.click(screen.getByRole('button', { name: 'Merge fields' }));
  await waitFor(() => expect(onClose).toHaveBeenCalledOnce());
  expect(wire).toHaveBeenCalledWith('domain.call', {
    operation: 'merge_entries',
    input: { mine: entryToInput(mine), theirs: entryToInput(theirs), fromTheirs: ['value'] }
  });
  expect(wire).toHaveBeenCalledWith('sync.resolve', {
    entryId: 'entry',
    resolution: 'merged',
    merged
  });
  view.unmount();
  expect(screen.queryByText('their-fictional-value')).not.toBeInTheDocument();
});
