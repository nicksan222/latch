import { LatchAgent } from '@latch/rpc/v1';
import { afterEach, expect, it } from 'vite-plus/test';
import { setTransport } from '~/lib/api/client';
import { setupDomainResponse } from '~/test/helpers';
afterEach(() => setTransport(null));
import { parseOtpauth } from './otpauth';

it('forwards the URI and preserves an absent Rust parse result', async () => {
  const call = setupDomainResponse({ operation: { case: 'parseOtpauth', value: {} } });
  expect(await parseOtpauth('invalid URI')).toBeUndefined();
  expect(call).toHaveBeenCalledWith(
    LatchAgent.method.domainCall,
    expect.objectContaining({
      operation: expect.objectContaining({
        case: 'parseOtpauth',
        value: expect.objectContaining({ value: 'invalid URI' })
      })
    })
  );
});
