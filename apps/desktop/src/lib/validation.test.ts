import { LatchAgent, EntryType } from '@latch/rpc/v1';
import { afterEach, expect, it } from 'vite-plus/test';
import { setTransport } from '~/lib/api/client';
import { setupDomainResponse } from '~/test/helpers';
afterEach(() => setTransport(null));
import { emptyEntryInput } from './entries';
import { recipientKeyKind, validateEntryInput } from './validation';

it('passes entry drafts to Rust and returns field paths unchanged', async () => {
  const output = { name: 'Name is required', 'totp.secret': 'Invalid TOTP secret' };
  const call = setupDomainResponse({
    operation: { case: 'validateEntry', value: { items: output } }
  });
  const input = emptyEntryInput('profile', EntryType.LOGIN);
  expect(await validateEntryInput(input)).toMatchObject(output);
  expect(call).toHaveBeenCalledWith(
    LatchAgent.method.domainCall,
    expect.objectContaining({
      operation: expect.objectContaining({ case: 'validateEntry', value: input })
    })
  );
});
it('does not classify recipient keys locally', async () => {
  const call = setupDomainResponse({ operation: { case: 'recipientKeyKind', value: {} } });
  expect(await recipientKeyKind('not a key')).toBeUndefined();
  expect(call).toHaveBeenCalledWith(
    LatchAgent.method.domainCall,
    expect.objectContaining({
      operation: expect.objectContaining({
        case: 'recipientKeyKind',
        value: expect.objectContaining({ value: 'not a key' })
      })
    })
  );
});
