import { create } from '@bufbuild/protobuf';
import {
  EntryType,
  EntryPayloadSchema,
  CustomFieldSchema,
  TotpConfigSchema,
  TotpAlgorithm,
  type EntryInput
} from '@latch/rpc/v1';
import { cleanup, fireEvent, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { useState } from 'react';
import { afterEach, beforeEach, describe, expect, it } from 'vite-plus/test';
import { emptyEntryInput } from '~/lib/entries';
import { validateEntryInput } from '~/lib/validation';
import { renderWithProviders, setupFixture } from '~/test/helpers';
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
      <output data-testid="state">
        {JSON.stringify(value, (_, field) => (typeof field === 'bigint' ? String(field) : field))}
      </output>
    </>
  );
}
const state = () => JSON.parse(screen.getByTestId('state').textContent ?? '{}') as EntryInput;

describe('entry-form', () => {
  it.each([
    EntryType.LOGIN,
    EntryType.SECRET,
    EntryType.ENV,
    EntryType.FILE,
    EntryType.NOTE,
    EntryType.SSH_KEY,
    EntryType.DATABASE,
    EntryType.API_KEY
  ] as const)('renders the %s form', (type) => {
    renderWithProviders(<Harness type={type} />);
    expect(screen.getByTestId('entry-form')).toBeInTheDocument();
    expect(screen.getByLabelText('Name')).toBeInTheDocument();
  });

  it('keeps custom field visibility attached to its row after deleting an earlier row', async () => {
    const user = userEvent.setup();
    const initial = emptyEntryInput('p', EntryType.LOGIN);
    if (initial.payload?.data.case !== 'login') throw new Error('Expected login');
    initial.payload!.data.value.custom = [
      create(CustomFieldSchema, { key: 'first', value: 'one', hidden: true }),
      create(CustomFieldSchema, { key: 'second', value: 'two', hidden: true })
    ];
    renderWithProviders(<Harness type={EntryType.LOGIN} initial={initial} />);
    const secondValue = screen.getByLabelText('Field 2 value');
    const secondRow = secondValue.closest('.grid')! as HTMLElement;
    await user.click(within(secondRow).getByRole('button', { name: 'Show value' }));
    expect(secondValue).toHaveAttribute('type', 'text');
    await user.click(screen.getByRole('button', { name: 'Remove field first' }));
    expect(screen.getByLabelText('Field 1 value')).toBe(secondValue);
    expect(secondValue).toHaveAttribute('type', 'text');
    const value = state();
    if (value.payload?.data.case !== 'login') throw new Error('Expected login');
    expect(value.payload!.data.value.custom).toEqual([
      create(CustomFieldSchema, { key: 'second', value: 'two', hidden: true })
    ]);
  });

  it('retains URL controls when an earlier duplicate row is removed', async () => {
    const user = userEvent.setup();
    const initial = emptyEntryInput('p', EntryType.LOGIN);
    if (initial.payload?.data.case !== 'login') throw new Error('Expected login');
    initial.payload!.data.value.urls = ['', ''];
    renderWithProviders(<Harness type={EntryType.LOGIN} initial={initial} />);
    const second = screen.getByLabelText('URL 2');
    await user.type(second, 'https://example.com');
    expect(screen.getByLabelText('URL 2')).toBe(second);
    await user.click(screen.getByRole('button', { name: 'Remove URL 1' }));
    expect(screen.getByLabelText('URL 1')).toBe(second);
    expect(second).toHaveValue('https://example.com');
  });

  it('fills TOTP from a pasted otpauth:// URI, including the username', async () => {
    renderWithProviders(<Harness type={EntryType.LOGIN} />);
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
    expect(s.payload?.data.case).toBe('login');
    if (s.payload?.data.case !== 'login') return;
    expect(s.payload!.data.value.totp).toEqual(
      create(TotpConfigSchema, {
        secret: 'JBSWY3DPEHPK3PXP',
        algorithm: TotpAlgorithm.SHA1,
        digits: 6,
        period: 30,
        issuer: 'GitHub'
      })
    );
    expect(s.payload!.data.value.username).toBe('nick@acme.dev');
    expect(screen.getByLabelText('TOTP secret')).toHaveValue('JBSWY3DPEHPK3PXP');
  });

  it('shows validation errors next to fields', async () => {
    const user = userEvent.setup();
    renderWithProviders(<Harness type={EntryType.LOGIN} />);
    await user.click(screen.getByRole('button', { name: 'Add TOTP' }));
    await user.type(screen.getByLabelText('TOTP secret'), 'not base32!');
    await user.click(screen.getByRole('button', { name: 'validate' }));
    expect(await screen.findByText('Name is required')).toBeInTheDocument();
    expect(screen.getByText('TOTP secret must be base32')).toBeInTheDocument();
    expect(screen.getByLabelText('Name')).toHaveAttribute('aria-invalid', 'true');
  });

  it('edits database fields and generates passwords', async () => {
    const user = userEvent.setup();
    renderWithProviders(<Harness type={EntryType.DATABASE} />);
    await user.type(screen.getByLabelText('Host'), 'db.local');
    await user.click(screen.getByRole('button', { name: 'Generate password' }));
    await waitFor(() => {
      const value = state();
      if (value.payload?.data.case !== 'database') throw new Error('type');
      expect(value.payload!.data.value.password).toHaveLength(24);
    });
    const s = state();
    if (s.payload?.data.case !== 'database') throw new Error('type');
    expect(s.payload!.data.value.host).toBe('db.local');
    expect(s.payload!.data.value.password).toHaveLength(24);
    expect(s.payload!.data.value.port).toBe(5432);
  });

  it('clearing an optional SSH passphrase sends "" (clear), untouched stays as loaded', async () => {
    const user = userEvent.setup();
    const initial: EntryInput = {
      ...emptyEntryInput('p', EntryType.SSH_KEY),
      name: 'key',
      payload: create(EntryPayloadSchema, {
        data: { case: 'sshKey', value: { publicKey: '', passphrase: 'old-pass', comment: '' } }
      })
    };
    renderWithProviders(<Harness initial={initial} type={EntryType.SSH_KEY} />);
    await user.clear(screen.getByLabelText('Key passphrase'));
    const s = state();
    if (s.payload?.data.case !== 'sshKey') throw new Error('type');
    expect(s.payload!.data.value.passphrase).toBe('');
    // The untouched (redacted) private key stays absent = keep.
    expect(s.payload!.data.value.privateKey).toBeUndefined();
  });

  it('clearing an optional API secret sends ""; a never-set secret stays absent', async () => {
    const user = userEvent.setup();
    const initial: EntryInput = {
      ...emptyEntryInput('p', EntryType.API_KEY),
      name: 'api',
      payload: create(EntryPayloadSchema, {
        data: { case: 'apiKey', value: { key: 'k', secret: 'old-secret' } }
      })
    };
    renderWithProviders(<Harness initial={initial} type={EntryType.API_KEY} />);
    await user.clear(screen.getByLabelText('Secret (optional)'));
    let s = state();
    if (s.payload?.data.case !== 'apiKey') throw new Error('type');
    expect(s.payload!.data.value.secret).toBe('');
    await user.type(screen.getByLabelText('Secret (optional)'), 'n');
    s = state();
    if (s.payload?.data.case !== 'apiKey') throw new Error('type');
    expect(s.payload!.data.value.secret).toBe('n');
  });

  it('an untouched optional secret on a new entry stays absent', () => {
    renderWithProviders(<Harness type={EntryType.API_KEY} />);
    const s = state();
    if (s.payload?.data.case !== 'apiKey') throw new Error('type');
    expect(s.payload!.data.value.secret).toBeUndefined();
  });
});
