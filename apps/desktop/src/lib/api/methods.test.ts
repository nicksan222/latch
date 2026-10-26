import type { DescMethodUnary, MessageInitShape } from '@bufbuild/protobuf';
import type { LatchAgent, EmptySchema, RevealParamsSchema } from '@latch/rpc/v1';
import { type Empty, type Entry, type VaultStatus } from '@latch/rpc/v1';
import { expect, expectTypeOf, it } from 'vitest';
import { METHODS } from './methods';
import type { Method, ParamsArg, ParamsOf, ResultOf } from './methods';

it('covers the generated service descriptor and preserves RPC call argument contracts', () => {
  expect(new Set(METHODS).size).toBe(METHODS.length);
  expectTypeOf<(typeof METHODS)[number]>().toEqualTypeOf<(typeof LatchAgent.methods)[number]>();
  expect(METHODS.every((method) => method.methodKind === 'unary')).toBe(true);
  expectTypeOf<Method>().toEqualTypeOf<DescMethodUnary>();
  expectTypeOf<ParamsArg<typeof LatchAgent.method.vaultStatus>>().toEqualTypeOf<
    [params?: MessageInitShape<typeof EmptySchema>]
  >();
  expectTypeOf<ParamsArg<typeof LatchAgent.method.totpPinned>>().toEqualTypeOf<
    [params?: MessageInitShape<typeof EmptySchema>]
  >();
  expectTypeOf<ParamsArg<typeof LatchAgent.method.vaultTrust>>().toEqualTypeOf<
    [params?: MessageInitShape<typeof EmptySchema>]
  >();
  expectTypeOf<ParamsOf<typeof LatchAgent.method.entriesReveal>>().toEqualTypeOf<
    MessageInitShape<typeof RevealParamsSchema>
  >();
  expectTypeOf<ResultOf<typeof LatchAgent.method.vaultLock>>().toEqualTypeOf<VaultStatus>();
  expectTypeOf<ResultOf<typeof LatchAgent.method.entriesGet>>().toEqualTypeOf<Entry>();
  expectTypeOf<ResultOf<typeof LatchAgent.method.vaultChangePassphrase>>().toEqualTypeOf<Empty>();
  expectTypeOf<ResultOf<typeof LatchAgent.method.profilesDelete>>().toEqualTypeOf<Empty>();
  expectTypeOf<ResultOf<typeof LatchAgent.method.entriesDelete>>().toEqualTypeOf<Empty>();
  expectTypeOf<ResultOf<typeof LatchAgent.method.browserClose>>().toEqualTypeOf<Empty>();
  expectTypeOf<ResultOf<typeof LatchAgent.method.browserReset>>().toEqualTypeOf<Empty>();
  // @ts-expect-error -- unknown IDs cannot select a schema
  expectTypeOf<ParamsOf<typeof LatchAgent.method.missingMethod>>();
  // @ts-expect-error -- parameterized methods require an argument
  expectTypeOf<ParamsArg<typeof LatchAgent.method.entriesGet>>().toEqualTypeOf<[]>();
});
