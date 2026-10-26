import { afterEach, expect, it } from 'vite-plus/test';
import { setTransport } from '~/lib/api/client';
import { setupDomainResponse } from '~/test/helpers';
afterEach(() => setTransport(null));
import { emptyEntryInput } from './entries';
import { recipientKeyKind, validateEntryInput } from './validation';

it('passes entry drafts to Rust and returns field paths unchanged', async () => {
  const output = { name: 'Name is required', 'totp.secret': 'Invalid TOTP secret' };
  const call = setupDomainResponse({ operation: 'validate_entry', output });
  const input = emptyEntryInput('profile', 'login');
  expect(await validateEntryInput(input)).toEqual(output);
  expect(call).toHaveBeenCalledWith('domain.call', { operation: 'validate_entry', input });
});
it('does not classify recipient keys locally', async () => {
  const call = setupDomainResponse({ operation: 'recipient_key_kind', output: null });
  expect(await recipientKeyKind('not a key')).toBeNull();
  expect(call).toHaveBeenCalledWith('domain.call', {
    operation: 'recipient_key_kind',
    input: 'not a key'
  });
});
