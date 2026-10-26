import { ErrorCode } from '@latch/rpc/v1';
import { afterEach, expect, it } from 'vite-plus/test';
import { setTransport } from '~/lib/api/client';
import { setupDomainResponse } from '~/test/helpers';
afterEach(() => setTransport(null));
import { domain } from './domain';

it('fails closed when the response operation does not match', async () => {
  setupDomainResponse({ operation: { case: 'parseOtpauth', value: {} } });
  await expect(domain('parseDotenv', { value: 'SECRET=fixture' })).rejects.toMatchObject({
    code: ErrorCode.INTERNAL,
    message: 'Unexpected domain response'
  });
});
