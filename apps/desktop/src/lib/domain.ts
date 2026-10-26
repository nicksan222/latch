import type { DomainRequest, DomainResponse } from '../_generated/contracts';
import { call } from './api/client';
import { RpcError } from './api/errors';

type Operation = DomainRequest['operation'];
type Input<K extends Operation> = Extract<DomainRequest, { operation: K }>['input'];
type Output<K extends Operation> = Extract<DomainResponse, { operation: K }>['output'];

/** Domain policy lives in Rust. This adapter only checks the response tag. */
export async function domain<K extends Operation>(
  operation: K,
  input: Input<K>
): Promise<Output<K>> {
  const response = await call('domain.call', { operation, input } as DomainRequest);
  if (response.operation !== operation) {
    throw new RpcError('internal', 'Unexpected domain response');
  }
  return response.output as Output<K>;
}
