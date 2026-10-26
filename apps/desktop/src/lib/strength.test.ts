import { LatchAgent } from '@latch/rpc/v1';
import { afterEach, expect, it } from 'vite-plus/test';
import { setTransport } from '~/lib/api/client';
import { setupDomainResponse } from '~/test/helpers';
afterEach(() => setTransport(null));
import { estimateStrength } from './strength';

it('returns the Rust strength result without a client estimate', async () => {
  const output = { score: 3, label: 'Strong', bits: 72, hints: [] };
  const call = setupDomainResponse({ operation: { case: 'estimateStrength', value: output } });
  expect(await estimateStrength('fictional phrase')).toMatchObject(output);
  expect(call).toHaveBeenCalledWith(
    LatchAgent.method.domainCall,
    expect.objectContaining({
      operation: expect.objectContaining({
        case: 'estimateStrength',
        value: expect.objectContaining({ value: 'fictional phrase' })
      })
    })
  );
});
