import { afterEach, expect, it } from 'vite-plus/test';
import { setTransport } from '~/lib/api/client';
import { setupDomainResponse } from '~/test/helpers';
afterEach(() => setTransport(null));
import { domain } from './domain';

it('fails closed when the response operation does not match', async () => {
  setupDomainResponse({ operation: 'parse_otpauth', output: null });
  await expect(domain('parse_dotenv', 'SECRET=fixture')).rejects.toMatchObject({
    code: 'internal',
    message: 'Unexpected domain response'
  });
});
