import { expect, expectTypeOf, it } from 'vitest';
import { METHODS, type MethodMap, type Entry, type VaultStatus } from '../../_generated/contracts';
import type { Method, ParamsArg, ParamsOf, ResultOf } from './methods';

it('covers the generated map and preserves RPC call argument contracts', () => {
  expect(new Set(METHODS).size).toBe(METHODS.length);
  expectTypeOf<(typeof METHODS)[number]>().toEqualTypeOf<keyof MethodMap>();
  expectTypeOf<Method>().toEqualTypeOf<keyof MethodMap>();
  expectTypeOf<ParamsArg<'vault.status'>>().toEqualTypeOf<[]>();
  expectTypeOf<ParamsArg<'totp.pinned'>>().toEqualTypeOf<[]>();
  expectTypeOf<ParamsArg<'vault.trust'>>().toEqualTypeOf<[params: Record<string, never>]>();
  expectTypeOf<ParamsOf<'entries.reveal'>>().toEqualTypeOf<{ id: string; field: string }>();
  expectTypeOf<ResultOf<'vault.lock'>>().toEqualTypeOf<VaultStatus>();
  expectTypeOf<ResultOf<'entries.get'>>().toEqualTypeOf<Entry>();
  expectTypeOf<ResultOf<'vault.changePassphrase'>>().toEqualTypeOf<null>();
  expectTypeOf<ResultOf<'profiles.delete'>>().toEqualTypeOf<null>();
  expectTypeOf<ResultOf<'entries.delete'>>().toEqualTypeOf<null>();
  expectTypeOf<ResultOf<'browser.close'>>().toEqualTypeOf<null>();
  expectTypeOf<ResultOf<'browser.reset'>>().toEqualTypeOf<null>();
  // @ts-expect-error -- unknown IDs cannot select a schema
  expectTypeOf<ParamsOf<'missing.method'>>();
  // @ts-expect-error -- parameterized methods require an argument
  expectTypeOf<ParamsArg<'entries.get'>>().toEqualTypeOf<[]>();
});
