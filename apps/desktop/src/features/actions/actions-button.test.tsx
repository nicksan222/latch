import { create, fromBinary, toBinary, type DescMethodUnary } from '@bufbuild/protobuf';
import { LatchAgent, EntryType } from '@latch/rpc/v1';
import { cleanup, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, expect, it, vi } from 'vite-plus/test';
import { setTransport } from '~/lib/api/client';
import { renderWithProviders } from '~/test/helpers';
import { ActionsButton } from './actions-button';

const request = vi.fn();
beforeEach(() => {
  request.mockReset();
  request.mockImplementation(async (method: DescMethodUnary) => {
    if (method === LatchAgent.method.actionsList)
      return { actions: [], workingDirectory: undefined };
    if (method === LatchAgent.method.entriesList)
      return { items: [{ id: 'env1', name: 'Development env', type: EntryType.ENV }] };
    return {};
  });
  setTransport({
    kind: 'stub',
    request: async (method, payload) =>
      toBinary(
        method.output,
        create(method.output, await request(method, fromBinary(method.input, payload)))
      ),
    subscribe: () => () => {}
  });
});
afterEach(() => {
  cleanup();
  setTransport(null);
});

it('creates a browser-only action with explicit env sources and optional key allowlist', async () => {
  const user = userEvent.setup();
  renderWithProviders(<ActionsButton profileId="p1" />);
  await user.click(screen.getByRole('button', { name: 'Actions' }));
  expect(await screen.findByText('No actions yet')).toBeInTheDocument();
  await user.click(screen.getByRole('button', { name: 'New action' }));
  await user.type(screen.getByLabelText('Name'), 'Open project');
  await user.type(screen.getByLabelText('Browser URLs'), 'https://example.com');
  await user.click(await screen.findByRole('checkbox', { name: 'Development env' }));
  await user.type(screen.getByLabelText('Variable names'), 'API_KEY, PORT');
  await user.click(screen.getByRole('button', { name: 'Save action' }));
  await waitFor(() =>
    expect(request).toHaveBeenCalledWith(
      LatchAgent.method.actionsSave,
      create(LatchAgent.method.actionsSave.input, {
        profileId: 'p1',
        id: undefined,
        input: {
          name: 'Open project',
          command: '',
          envEntryIds: ['env1'],
          envKeys: { items: ['API_KEY', 'PORT'] },
          urls: ['https://example.com']
        }
      })
    )
  );
  expect(
    request.mock.calls.some(
      ([method]) =>
        method === LatchAgent.method.entriesReveal || method === LatchAgent.method.entriesGet
    )
  ).toBe(false);
});

it('runs saved IDs, edits definitions and confirms deletion', async () => {
  request.mockImplementation(async (method: DescMethodUnary) =>
    method === LatchAgent.method.actionsList
      ? {
          actions: [
            {
              id: 'a1',
              name: 'Develop',
              command: 'bun run dev',
              envEntryIds: [],
              envKeys: undefined,
              urls: []
            }
          ],
          workingDirectory: '/project'
        }
      : method === LatchAgent.method.entriesList
        ? { items: [] }
        : {}
  );
  const user = userEvent.setup();
  renderWithProviders(<ActionsButton profileId="p1" />);
  await user.click(screen.getByRole('button', { name: 'Actions' }));
  await user.click(await screen.findByRole('button', { name: 'Play Develop' }));
  await waitFor(() =>
    expect(request).toHaveBeenCalledWith(
      LatchAgent.method.actionsRun,
      create(LatchAgent.method.actionsRun.input, { profileId: 'p1', actionId: 'a1' })
    )
  );
  await user.click(screen.getByRole('button', { name: 'Edit Develop' }));
  expect(screen.getByLabelText('Command')).toHaveValue('bun run dev');
  await user.click(screen.getByRole('button', { name: 'Cancel' }));
  await user.click(screen.getByRole('button', { name: 'Delete Develop' }));
  expect(request.mock.calls.some(([method]) => method === LatchAgent.method.actionsDelete)).toBe(
    false
  );
  await user.click(screen.getByRole('button', { name: 'Delete action' }));
  await waitFor(() =>
    expect(request).toHaveBeenCalledWith(
      LatchAgent.method.actionsDelete,
      create(LatchAgent.method.actionsDelete.input, { profileId: 'p1', actionId: 'a1' })
    )
  );
});

it('saves the machine-local directory separately from shared definitions', async () => {
  const user = userEvent.setup();
  renderWithProviders(<ActionsButton profileId="p1" />);
  await user.click(screen.getByRole('button', { name: 'Actions' }));
  await user.type(await screen.findByLabelText('Project directory'), '/home/me/project');
  await user.click(screen.getByRole('button', { name: 'Save directory' }));
  await waitFor(() =>
    expect(request).toHaveBeenCalledWith(
      LatchAgent.method.actionsConfigure,
      create(LatchAgent.method.actionsConfigure.input, {
        profileId: 'p1',
        workingDirectory: '/home/me/project'
      })
    )
  );
});

it('shows load failures with a retry control', async () => {
  request.mockImplementation(async () => {
    throw new Error('Agent unavailable');
  });
  const user = userEvent.setup();
  renderWithProviders(<ActionsButton profileId="p1" />);
  await user.click(screen.getByRole('button', { name: 'Actions' }));
  expect(await screen.findByRole('alert')).toHaveTextContent('Agent unavailable');
  request.mockResolvedValue({ actions: [], workingDirectory: undefined });
  await user.click(screen.getByRole('button', { name: 'Retry' }));
  expect(await screen.findByText('No actions yet')).toBeInTheDocument();
});

it('disables launch controls while a run is pending and surfaces RPC errors', async () => {
  let rejectRun: (error: Error) => void = () => {};
  request.mockImplementation(async (method: DescMethodUnary) => {
    if (method === LatchAgent.method.actionsList)
      return {
        actions: [
          {
            id: 'a1',
            name: 'Develop',
            command: '',
            envEntryIds: [],
            envKeys: undefined,
            urls: ['https://example.com']
          }
        ],
        workingDirectory: undefined
      };
    if (method === LatchAgent.method.actionsRun)
      return new Promise((_resolve, reject) => {
        rejectRun = reject;
      });
    return {};
  });
  const user = userEvent.setup();
  renderWithProviders(<ActionsButton profileId="p1" />);
  await user.click(screen.getByRole('button', { name: 'Actions' }));
  await user.click(await screen.findByRole('button', { name: 'Play Develop' }));
  expect(screen.getByRole('button', { name: 'Play Develop' })).toBeDisabled();
  expect(screen.getByRole('button', { name: 'Edit Develop' })).toBeDisabled();
  rejectRun(new Error('Browser is already running'));
  expect(await screen.findByText('Could not launch action')).toBeInTheDocument();
  expect(await screen.findByText('Browser is already running')).toBeInTheDocument();
  await waitFor(() => expect(screen.getByRole('button', { name: 'Play Develop' })).toBeEnabled());
});

it('saves edited actions by ID and omits a blank allowlist', async () => {
  request.mockImplementation(async (method: DescMethodUnary) =>
    method === LatchAgent.method.actionsList
      ? {
          actions: [
            {
              id: 'a1',
              name: 'Develop',
              command: 'bun run dev',
              envEntryIds: [],
              envKeys: { items: ['PORT'] },
              urls: []
            }
          ],
          workingDirectory: '/project'
        }
      : method === LatchAgent.method.entriesList
        ? { items: [] }
        : {}
  );
  const user = userEvent.setup();
  renderWithProviders(<ActionsButton profileId="p1" />);
  await user.click(screen.getByRole('button', { name: 'Actions' }));
  await user.click(await screen.findByRole('button', { name: 'Edit Develop' }));
  await user.clear(screen.getByLabelText('Variable names'));
  await user.click(screen.getByRole('button', { name: 'Save action' }));
  await waitFor(() =>
    expect(request).toHaveBeenCalledWith(
      LatchAgent.method.actionsSave,
      create(LatchAgent.method.actionsSave.input, {
        profileId: 'p1',
        id: 'a1',
        input: {
          name: 'Develop',
          command: 'bun run dev',
          envEntryIds: [],
          envKeys: undefined,
          urls: []
        }
      })
    )
  );
});
