import { afterEach, expect, it } from 'vite-plus/test';
import { setTransport } from '~/lib/api/client';
import { setupDomainResponse } from '~/test/helpers';
afterEach(() => setTransport(null));
import { estimateStrength } from './strength';

it('returns the Rust strength result without a client estimate', async () => {
  const output = { score: 3, label: 'Strong', bits: 72, hints: [] };
  const call = setupDomainResponse({ operation: 'estimate_strength', output });
  expect(await estimateStrength('fictional phrase')).toEqual(output);
  expect(call).toHaveBeenCalledWith('domain.call', {
    operation: 'estimate_strength',
    input: 'fictional phrase'
  });
});
