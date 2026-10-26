import { create, fromBinary, toBinary, type MessageInitShape } from '@bufbuild/protobuf';
import {
  LatchAgent,
  ErrorCode,
  type Entry,
  type EntriesListResponse,
  type Empty,
  type VaultStatus
} from '@latch/rpc/v1';
import { afterEach, describe, expect, expectTypeOf, it, vi } from 'vite-plus/test';
import { call, setTransport } from '../lib/api/client';
import type { ParamsArg, ParamsOf, ResultOf } from '../lib/api/methods';

afterEach(() => setTransport(null));

describe('descriptor client binary schema roundtrips', () => {
  it('roundtrips parameterless status through its official schema', async () => {
    const method = LatchAgent.method.vaultStatus;
    const response = create(method.output, { initialized: true, locked: false });
    const request = vi.fn(async (descriptor, bytes) => {
      expect(descriptor).toBe(method);
      expect(fromBinary(method.input, bytes)).toEqual(create(method.input));
      return toBinary(method.output, response);
    });
    setTransport({ kind: 'stub', request, subscribe: () => () => {} });
    await expect(call(method)).resolves.toEqual(response);
    expect(request).toHaveBeenCalledTimes(1);
  });

  it('roundtrips named entry creation parameters and the full result', async () => {
    const method = LatchAgent.method.entriesCreate;
    const input: ParamsOf<typeof method> = {
      profileId: 'profile',
      name: 'Note',
      payload: { data: { case: 'note', value: { body: 'body' } } }
    };
    const response = create(method.output, {
      ...input,
      id: 'entry',
      createdAt: '2026-10-25T00:00:00Z'
    });
    setTransport({
      kind: 'stub',
      request: async (descriptor, bytes) => {
        expect(descriptor).toBe(method);
        expect(fromBinary(method.input, bytes)).toEqual(create(method.input, input));
        return toBinary(method.output, response);
      },
      subscribe: () => () => {}
    });
    await expect(call(method, input)).resolves.toEqual(response);
  });

  it('preserves uint64 precision and optional secret presence in binary messages', async () => {
    const method = LatchAgent.method.entriesCreate;
    const size = 9007199254740993n;
    setTransport({
      kind: 'stub',
      async request(descriptor, bytes) {
        const input = fromBinary(method.input, bytes);
        expect(descriptor).toBe(method);
        expect(input.payload?.data).toMatchObject({ case: 'file', value: { size } });
        return toBinary(
          method.output,
          create(method.output, { id: 'file', profileId: input.profileId, payload: input.payload })
        );
      },
      subscribe: () => () => {}
    });
    const entry = await call(method, {
      profileId: 'profile',
      payload: { data: { case: 'file', value: { filename: 'file', size } } }
    });
    expect(entry.payload?.data).toMatchObject({ case: 'file', value: { size } });

    const schema = LatchAgent.method.entriesUpdate.input;
    for (const password of [undefined, '']) {
      const request = create(schema, {
        id: 'login',
        input: { payload: { data: { case: 'login', value: { password } } } }
      });
      const decoded = fromBinary(schema, toBinary(schema, request));
      const data = decoded.input?.payload?.data;
      expect(data?.case).toBe('login');
      if (data?.case !== 'login') throw new Error('Expected login payload');
      expect(data.value.password).toBe(password);
      expect(Object.hasOwn(data.value, 'password')).toBe(password !== undefined);
    }
  });

  it('preserves list wrappers instead of unwrapping their items', async () => {
    const method = LatchAgent.method.profilesList;
    const response = create(method.output, { items: [{ id: 'profile', name: 'Personal' }] });
    setTransport({
      kind: 'stub',
      request: async () => toBinary(method.output, response),
      subscribe: () => () => {}
    });
    await expect(call(method)).resolves.toEqual(response);
  });

  it('roundtrips reveal values through the shared client boundary', async () => {
    const method = LatchAgent.method.entriesReveal;
    setTransport({
      kind: 'stub',
      request: async (_, bytes) => {
        expect(fromBinary(method.input, bytes).field).toBe('password');
        return toBinary(method.output, create(method.output, { value: 'revealed' }));
      },
      subscribe: () => () => {}
    });
    await expect(call(method, { id: 'entry', field: 'password' })).resolves.toMatchObject({
      value: 'revealed'
    });
  });

  it.each(['unknown.method', 'toString', '__proto__'])(
    'excludes unknown procedure %s from the descriptor catalog',
    (name) => {
      expect(
        LatchAgent.methods.some((method) => method.name === name || method.localName === name)
      ).toBe(false);
    }
  );

  it.each([{ code: ErrorCode.LOCKED, message: 'Vault is locked' }, 'native failure'])(
    'normalizes native errors for shared UI handling (%s)',
    async (error) => {
      setTransport({
        kind: 'stub',
        request: async () => {
          throw error;
        },
        subscribe: () => () => {}
      });
      await expect(call(LatchAgent.method.vaultStatus)).rejects.toMatchObject({
        code: typeof error === 'string' ? ErrorCode.INTERNAL : error.code,
        message: typeof error === 'string' ? error : error.message
      });
    }
  );

  it('types procedure requests and responses from official descriptors', () => {
    expectTypeOf<ResultOf<typeof LatchAgent.method.vaultStatus>>().toEqualTypeOf<VaultStatus>();
    expectTypeOf<ResultOf<typeof LatchAgent.method.entriesCreate>>().toEqualTypeOf<Entry>();
    expectTypeOf<
      ResultOf<typeof LatchAgent.method.entriesList>
    >().toEqualTypeOf<EntriesListResponse>();
    expectTypeOf<ResultOf<typeof LatchAgent.method.entriesDelete>>().toEqualTypeOf<Empty>();
    expectTypeOf<ParamsArg<typeof LatchAgent.method.entriesCreate>>().toEqualTypeOf<
      [params: MessageInitShape<typeof LatchAgent.method.entriesCreate.input>]
    >();
    // @ts-expect-error -- procedure requires a request
    void (() => call(LatchAgent.method.entriesCreate));
    // @ts-expect-error -- incorrect request shape
    void (() => call(LatchAgent.method.entriesCreate, { profileId: 1 }));
    void (() => call(LatchAgent.method.vaultStatus));
    void (() => call(LatchAgent.method.vaultStatus, {}));
    // @ts-expect-error -- parameterless procedure rejects unknown fields
    void (() => call(LatchAgent.method.vaultStatus, { invalid: true }));
    // @ts-expect-error -- nonexistent procedure
    void (() => call(LatchAgent.method.vaultNonexistent));
  });
});
