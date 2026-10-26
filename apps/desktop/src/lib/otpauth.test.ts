import { afterEach, expect, it } from 'vite-plus/test';
import { setTransport } from '~/lib/api/client';
import { setupDomainResponse } from '~/test/helpers';
afterEach(() => setTransport(null));
import { parseOtpauth } from './otpauth';

it('forwards the URI and preserves a null Rust parse result', async () => {
  const call = setupDomainResponse({ operation: 'parse_otpauth', output: null });
  expect(await parseOtpauth('invalid URI')).toBeNull();
  expect(call).toHaveBeenCalledWith('domain.call', {
    operation: 'parse_otpauth',
    input: 'invalid URI'
  });
});
