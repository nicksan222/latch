import { cleanup, fireEvent, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { useState } from 'react';
import { afterEach, beforeEach, describe, expect, it } from 'vite-plus/test';
import { emptyEntryInput } from '~/lib/entries';
import { validateEntryInput } from '~/lib/validation';
import { renderWithProviders, setupFixture } from '~/test/helpers';
import type { EntryInput, EntryType } from '../../_generated/contracts';
import { EntryForm } from './entry-form';

let backend: Awaited<ReturnType<typeof setupFixture>>;
beforeEach(async () => {
  backend = await setupFixture();
});
afterEach(async () => {
  cleanup();
  await backend.dispose();
});

function Harness({ type, initial }: { type: EntryType; initial?: EntryInput }) {
  const [value, setValue] = useState<EntryInput>(() => initial ?? emptyEntryInput('p', type));
  const [errors, setErrors] = useState({});
  return (
    <>
      <EntryForm errors={errors} value={value} onChange={setValue} />
      <button type="button" onClick={async () => setErrors(await validateEntryInput(value))}>
        validate
      </button>
      <output data-testid="state">{JSON.stringify(value)}</output>
    </>
  );
}
const state = () => JSON.parse(screen.getByTestId('state').textContent ?? '{}') as EntryInput;

describe('entry-form', () => {
  it.each(['login', 'secret', 'env', 'file', 'note', 'ssh_key', 'database', 'api_key'] as const)(
    'renders the %s form',
    (type) => {
      renderWithProviders(<Harness type={type} />);
      expect(screen.getByTestId('entry-form')).toBeInTheDocument();
      expect(screen.getByLabelText('Name')).toBeInTheDocument();
    }
  );

  it('keeps custom field visibility attached to its row after deleting an earlier row', async () => {
    const user = userEvent.setup();
    const initial = emptyEntryInput('p', 'login');
    if (initial.payload.type !== 'login') throw new Error('Expected login');
    initial.payload.data.custom = [
      { key: 'first', value: 'one', hidden: true },
      { key: 'second', value: 'two', hidden: true }
    ];
    renderWithProviders(<Harness type="login" initial={initial} />);
    const secondValue = screen.getByLabelText('Field 2 value');
    const secondRow = secondValue.closest('.grid')! as HTMLElement;
    await user.click(within(secondRow).getByRole('button', { name: 'Show value' }));
    expect(secondValue).toHaveAttribute('type', 'text');
    await user.click(screen.getByRole('button', { name: 'Remove field first' }));
    expect(screen.getByLabelText('Field 1 value')).toBe(secondValue);
    expect(secondValue).toHaveAttribute('type', 'text');
    const value = state();
    if (value.payload.type !== 'login') throw new Error('Expected login');
    expect(value.payload.data.custom).toEqual([{ key: 'second', value: 'two', hidden: true }]);
  });

  it('retains URL controls when an earlier duplicate row is removed', async () => {
    const user = userEvent.setup();
    const initial = emptyEntryInput('p', 'login');
    if (initial.payload.type !== 'login') throw new Error('Expected login');
    initial.payload.data.urls = ['', ''];
    renderWithProviders(<Harness type="login" initial={initial} />);
    const second = screen.getByLabelText('URL 2');
    await user.type(second, 'https://example.com');
    expect(screen.getByLabelText('URL 2')).toBe(second);
    await user.click(screen.getByRole('button', { name: 'Remove URL 1' }));
    expect(screen.getByLabelText('URL 1')).toBe(second);
    expect(second).toHaveValue('https://example.com');
  });

  it('fills TOTP from a pasted otpauth:// URI, including the username', async () => {
    renderWithProviders(<Harness type="login" />);
    const input = screen.getByRole('textbox', { name: 'Paste otpauth URI' });
    fireEvent.paste(input, {
      clipboardData: {
        getData: () =>
          'otpauth://totp/GitHub:nick@acme.dev?secret=JBSWY3DPEHPK3PXP&issuer=GitHub&digits=6'
      }
    });
    await waitFor(() =>
      expect(screen.getByLabelText('TOTP secret')).toHaveValue('JBSWY3DPEHPK3PXP')
    );
    const s = state();
    expect(s.payload.type).toBe('login');
    if (s.payload.type !== 'login') return;
    expect(s.payload.data.totp).toEqual({
      secret: 'JBSWY3DPEHPK3PXP',
      algorithm: 'SHA1',
      digits: 6,
      period: 30,
      issuer: 'GitHub'
    });
    expect(s.payload.data.username).toBe('nick@acme.dev');
    expect(screen.getByLabelText('TOTP secret')).toHaveValue('JBSWY3DPEHPK3PXP');
  });

  it('shows validation errors next to fields', async () => {
    const user = userEvent.setup();
    renderWithProviders(<Harness type="login" />);
    await user.click(screen.getByRole('button', { name: 'Add TOTP' }));
    await user.type(screen.getByLabelText('TOTP secret'), 'not base32!');
    await user.click(screen.getByRole('button', { name: 'validate' }));
    expect(await screen.findByText('Name is required')).toBeInTheDocument();
    expect(screen.getByText('TOTP secret must be base32')).toBeInTheDocument();
    expect(screen.getByLabelText('Name')).toHaveAttribute('aria-invalid', 'true');
  });

  it('edits database fields and generates passwords', async () => {
    const user = userEvent.setup();
    renderWithProviders(<Harness type="database" />);
    await user.type(screen.getByLabelText('Host'), 'db.local');
    await user.click(screen.getByRole('button', { name: 'Generate password' }));
    await waitFor(() => {
      const value = state();
      if (value.payload.type !== 'database') throw new Error('type');
      expect(value.payload.data.password).toHaveLength(24);
    });
    const s = state();
    if (s.payload.type !== 'database') throw new Error('type');
    expect(s.payload.data.host).toBe('db.local');
    expect(s.payload.data.password).toHaveLength(24);
    expect(s.payload.data.port).toBe(5432);
  });

  it('clearing an optional SSH passphrase sends "" (clear), untouched stays as loaded', async () => {
    const user = userEvent.setup();
    const initial: EntryInput = {
      ...emptyEntryInput('p', 'ssh_key'),
      name: 'key',
      payload: {
        type: 'ssh_key',
        data: { privateKey: null, publicKey: '', passphrase: 'old-pass', comment: '' }
      }
    };
    renderWithProviders(<Harness initial={initial} type="ssh_key" />);
    await user.clear(screen.getByLabelText('Key passphrase'));
    const s = state();
    if (s.payload.type !== 'ssh_key') throw new Error('type');
    expect(s.payload.data.passphrase).toBe('');
    // The untouched (redacted) private key stays null = keep.
    expect(s.payload.data.privateKey).toBeNull();
  });

  it('clearing an optional API secret sends ""; a never-set secret stays null', async () => {
    const user = userEvent.setup();
    const initial: EntryInput = {
      ...emptyEntryInput('p', 'api_key'),
      name: 'api',
      payload: {
        type: 'api_key',
        data: { key: 'k', secret: 'old-secret', endpoint: null, expiresAt: null }
      }
    };
    renderWithProviders(<Harness initial={initial} type="api_key" />);
    await user.clear(screen.getByLabelText('Secret (optional)'));
    let s = state();
    if (s.payload.type !== 'api_key') throw new Error('type');
    expect(s.payload.data.secret).toBe('');
    await user.type(screen.getByLabelText('Secret (optional)'), 'n');
    s = state();
    if (s.payload.type !== 'api_key') throw new Error('type');
    expect(s.payload.data.secret).toBe('n');
  });

  it('an untouched optional secret on a new entry stays null', () => {
    renderWithProviders(<Harness type="api_key" />);
    const s = state();
    if (s.payload.type !== 'api_key') throw new Error('type');
    expect(s.payload.data.secret).toBeNull();
  });
});
