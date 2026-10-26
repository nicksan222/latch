import { create } from '@bufbuild/protobuf';
import { EnvVarSchema, LatchAgent } from '@latch/rpc/v1';
import type { EnvVar } from '@latch/rpc/v1';
import { cleanup, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { useState } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';
import { setTransport } from '~/lib/api/client';
import type { FixtureClient } from '~/lib/api/fixture';
import { entryId, renderWithProviders, setupFixture } from '~/test/helpers';
import { EnvEditor, mergeEnvVars, moveItem, runCommand } from './env-editor';

let backend: FixtureClient;
beforeEach(async () => {
  backend = await setupFixture();
});
afterEach(async () => {
  cleanup();
  await backend.dispose();
  setTransport(null);
});

const v = (key: string, value: string, hidden = false): EnvVar =>
  create(EnvVarSchema, {
    key,
    value,
    hidden,
    comment: undefined
  });

describe('helpers', () => {
  it('merges imported vars in place and appends new ones', () => {
    expect(mergeEnvVars([v('A', '1'), v('B', '2')], [v('B', '3'), v('C', '4')])).toEqual([
      v('A', '1'),
      v('B', '3'),
      v('C', '4')
    ]);
  });
  it('reorders', () => {
    expect(moveItem([1, 2, 3], 0, 1)).toEqual([2, 1, 3]);
    expect(moveItem([1, 2, 3], 0, -1)).toEqual([1, 2, 3]);
  });
  it('builds the CLI command, quoting when needed', () => {
    expect(runCommand('acme', 'AWS prod', 'bun run dev')).toBe(
      "latch env acme 'AWS prod' -- bun run dev"
    );
    expect(runCommand('side-project', null, '')).toBe('latch env side-project -- <command>');
  });
});

function Harness({ initial }: { initial: EnvVar[] }) {
  const [vars, setVars] = useState(initial);
  return (
    <>
      <EnvEditor editable vars={vars} onChange={setVars} />
      <output data-testid="state">{JSON.stringify(vars)}</output>
    </>
  );
}
const state = () => JSON.parse(screen.getByTestId('state').textContent ?? '[]') as EnvVar[];

describe('EnvEditor (editable)', () => {
  it('adds, edits, reorders and removes rows', async () => {
    const user = userEvent.setup();
    renderWithProviders(<Harness initial={[v('A', '1')]} />);
    await user.click(screen.getByRole('button', { name: 'Add variable' }));
    await user.type(screen.getByRole('textbox', { name: 'Variable 2 key' }), 'new key');
    expect(state()[1]).toMatchObject({ key: 'NEW_KEY', hidden: true });
    await user.type(screen.getByRole('textbox', { name: 'NEW_KEY value' }), 'xyz');
    await user.click(screen.getByRole('button', { name: 'Move NEW_KEY up' }));
    expect(state().map((x) => x.key)).toEqual(['NEW_KEY', 'A']);
    await user.click(screen.getByRole('checkbox', { name: 'Hide NEW_KEY' }));
    expect(state()[0]!.hidden).toBe(false);
    await user.click(screen.getByRole('button', { name: 'Remove A' }));
    expect(state()).toEqual([v('NEW_KEY', 'xyz', false)]);
  });

  it('moves the existing row controls with their values and keeps them when another row is removed', async () => {
    const user = userEvent.setup();
    renderWithProviders(<Harness initial={[v('A', 'one'), v('B', 'two')]} />);
    const second = screen.getByRole('textbox', { name: 'Variable 2 key' });
    await user.click(screen.getByRole('button', { name: 'Move B up' }));
    expect(screen.getByRole('textbox', { name: 'Variable 1 key' })).toBe(second);
    await user.click(screen.getByRole('button', { name: 'Remove A' }));
    expect(screen.getByRole('textbox', { name: 'Variable 1 key' })).toBe(second);
    expect(state()).toEqual([v('B', 'two')]);
  });

  it('imports a pasted .env through env.parse', async () => {
    const user = userEvent.setup();
    renderWithProviders(<Harness initial={[v('A', 'old')]} />);
    await user.click(screen.getByRole('button', { name: 'Import .env' }));
    const dialog = await screen.findByRole('dialog');
    await user.type(
      within(dialog).getByRole('textbox', { name: '.env contents' }),
      'A=new{Enter}API_TOKEN=tok{Enter}'
    );
    await user.click(within(dialog).getByRole('button', { name: 'Import' }));
    await waitFor(() =>
      expect(state()).toEqual([v('A', 'new', true), v('API_TOKEN', 'tok', true)])
    );
    expect(backend.calls.some((c) => c.method === LatchAgent.method.envParse.name)).toBe(true);
  });
});

describe('EnvEditor (saved entry)', () => {
  it('reveals hidden vars individually and shows the Run… command', async () => {
    const user = userEvent.setup();
    const id = await entryId(backend, 'AWS prod');
    renderWithProviders(
      <EnvEditor
        editable={false}
        entryId={id}
        entryName="AWS prod"
        profileId="p"
        profileName="acme"
        vars={[
          v('AWS_REGION', 'eu-west-1'),
          create(EnvVarSchema, { key: 'AWS_SECRET_ACCESS_KEY', hidden: true })
        ]}
      />
    );
    expect(screen.getByText('eu-west-1')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Reveal AWS_SECRET_ACCESS_KEY' }));
    await screen.findByDisplayValue('wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY');
    await user.click(screen.getByRole('button', { name: 'Run…' }));
    expect(await screen.findByTestId('run-command')).toHaveTextContent(
      "latch env acme 'AWS prod' -- bun run dev"
    );
  });

  it('Copy as .env goes through env.copy: no plaintext reaches the webview', async () => {
    const user = userEvent.setup();
    const id = await entryId(backend, 'AWS prod');
    const acmeId = backend.fixture.profiles.acme;
    const writeText = vi.fn(async () => {});
    Object.defineProperty(navigator, 'clipboard', { configurable: true, value: { writeText } });
    renderWithProviders(
      <EnvEditor
        editable={false}
        entryId={id}
        entryName="AWS prod"
        profileId={acmeId}
        profileName="acme"
        vars={[
          v('AWS_REGION', 'eu-west-1'),
          create(EnvVarSchema, { key: 'AWS_SECRET_ACCESS_KEY', hidden: true })
        ]}
      />
    );
    await user.click(screen.getByRole('button', { name: 'Copy as…' }));
    await user.click(await screen.findByRole('menuitem', { name: 'Copy as .env' }));
    await waitFor(() => expect(backend.clipboard).toContain('AWS_SECRET_ACCESS_KEY='));
    expect(backend.clipboard).toContain('AWS_SECRET_ACCESS_KEY=');
    const methods = backend.calls.map((c) => c.method);
    expect(methods).toContain(LatchAgent.method.envCopy.name);
    expect(methods).not.toContain(LatchAgent.method.envFormat.name);
    expect(methods).not.toContain(LatchAgent.method.entriesReveal.name);
    expect(writeText).not.toHaveBeenCalled();
    expect(await screen.findByText('Clipboard clears in 30s')).toBeInTheDocument();
    expect(screen.queryByText(/Clear your clipboard/)).toBeNull();
  });
});
