import type { MessageInitShape } from '@bufbuild/protobuf';
import { ErrorCode } from '@latch/rpc/v1';
import {
  type DomainRequestSchema,
  LatchAgent,
  type DomainRequest,
  type DomainResponse
} from '@latch/rpc/v1';
import { call } from './api/client';
import { RpcError } from './api/errors';

type Operation = Exclude<DomainRequest['operation']['case'], undefined>;
type Input<K extends Operation> = Extract<
  MessageInitShape<typeof DomainRequestSchema>['operation'],
  { case: K }
>['value'];
type Output<K extends Operation> = Extract<DomainResponse['operation'], { case: K }>['value'];

/** Domain policy lives in Rust. This adapter checks the protobuf response discriminant. */
export async function domain<K extends Operation>(
  operation: K,
  input: Input<K>
): Promise<Output<K>> {
  const response = await call(LatchAgent.method.domainCall, {
    operation: { case: operation, value: input }
  } as MessageInitShape<typeof DomainRequestSchema>);
  if (response.operation.case !== operation)
    throw new RpcError(ErrorCode.INTERNAL, 'Unexpected domain response');
  return response.operation.value as Output<K>;
}
