import { create, fromBinary, toBinary, type DescMethodUnary } from '@bufbuild/protobuf';
import {
  ConflictDetailSchema,
  ConflictSummarySchema,
  DomainDiffRowListSchema,
  DomainRequestSchema,
  DomainResponseSchema,
  EntrySchema,
  EntryPayloadSchema,
  EntryInputSchema,
  LatchAgent,
  Resolution
} from '@latch/rpc/v1';
import type { DomainRequest, Entry } from '@latch/rpc/v1';
import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, expect, it, vi } from 'vite-plus/test';
import { setTransport } from '~/lib/api/client';
import { entryToInput } from '~/lib/entries';
import { renderWithProviders } from '~/test/helpers';
afterEach(() => setTransport(null));
import { ConflictResolver, diffRows } from './conflict-resolver';

it('requests the Rust diff and preserves content-only difference metadata', async () => {
  const output = [
    { field: 'file', mine: 'id.pem', theirs: 'id.pem', differs: true, contentDiffers: true }
  ];
  const call = vi.fn();
  setTransport({
    kind: 'stub',
    request: async (method, payload) => {
      call(method, fromBinary(method.input, payload));
      return toBinary(
        DomainResponseSchema,
        create(DomainResponseSchema, { operation: { case: 'diffRows', value: { items: output } } })
      );
    },
    subscribe: () => () => {}
  });
  expect(await diffRows(create(ConflictDetailSchema, { entryId: 'e' }))).toEqual(
    create(DomainDiffRowListSchema, { items: output }).items
  );
  expect(call).toHaveBeenCalledWith(
    LatchAgent.method.domainCall,
    create(DomainRequestSchema, {
      operation: { case: 'diffRows', value: {} }
    })
  );
});

it('selects fields, requests a Rust merge, and submits the returned draft', async () => {
  const user = userEvent.setup();
  const mine: Entry = create(EntrySchema, {
    id: 'entry',
    profileId: 'profile',
    name: 'GitHub',
    tags: [],
    notes: 'mine',
    favorite: false,
    createdAt: '2026-01-01T00:00:00Z',
    updatedAt: '2026-01-01T00:00:00Z',
    redactedFields: [],
    payload: { data: { case: 'secret', value: { value: 'my-fictional-value' } } }
  });
  const theirs: Entry = create(EntrySchema, {
    ...mine,
    notes: 'theirs',
    payload: create(EntryPayloadSchema, {
      data: { case: 'secret', value: { value: 'their-fictional-value' } }
    })
  });
  const merged = create(EntryInputSchema, { ...entryToInput(mine), payload: theirs.payload });
  const wire = vi.fn(async (method: DescMethodUnary, params: unknown) => {
    if (method === LatchAgent.method.syncConflict) return { entryId: 'entry', mine, theirs };
    if (method === LatchAgent.method.domainCall) {
      const request = params as DomainRequest;
      if (request.operation.case === 'diffRows')
        return {
          operation: {
            case: 'diffRows',
            value: {
              items: [
                {
                  field: 'value',
                  mine: 'my-fictional-value',
                  theirs: 'their-fictional-value',
                  differs: true,
                  contentDiffers: false
                },
                {
                  field: 'notes',
                  mine: 'mine',
                  theirs: 'theirs',
                  differs: true,
                  contentDiffers: false
                }
              ]
            }
          }
        };
      if (request.operation.case === 'mergeEntries')
        return { operation: { case: 'mergeEntries', value: merged } };
    }
    if (method === LatchAgent.method.syncResolve) return {};
    throw new Error(`Unexpected test RPC ${method}`);
  });
  setTransport({
    kind: 'stub',
    request: async (method, payload) =>
      toBinary(
        method.output,
        create(method.output, await wire(method, fromBinary(method.input, payload)))
      ),
    subscribe: () => () => {}
  });
  const onClose = vi.fn();
  const view = renderWithProviders(
    <ConflictResolver
      conflict={create(ConflictSummarySchema, {
        entryId: 'entry',
        profileId: 'profile',
        name: 'GitHub'
      })}
      onClose={onClose}
    />
  );
  await user.click(await screen.findByRole('radio', { name: 'Take their value' }));
  expect(screen.getByRole('radio', { name: 'Keep my notes' })).toBeChecked();
  await user.click(screen.getByRole('button', { name: 'Merge fields' }));
  await waitFor(() => expect(onClose).toHaveBeenCalledOnce());
  expect(wire).toHaveBeenCalledWith(
    LatchAgent.method.domainCall,
    create(DomainRequestSchema, {
      operation: {
        case: 'mergeEntries',
        value: { mine: entryToInput(mine), theirs: entryToInput(theirs), fromTheirs: ['value'] }
      }
    })
  );
  expect(wire).toHaveBeenCalledWith(
    LatchAgent.method.syncResolve,
    create(LatchAgent.method.syncResolve.input, {
      entryId: 'entry',
      resolution: Resolution.MERGED,
      merged
    })
  );
  view.unmount();
  expect(screen.queryByText('their-fictional-value')).not.toBeInTheDocument();
});
