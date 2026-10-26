import { create } from '@bufbuild/protobuf';
import { EntryType, EntryPayloadSchema, LatchAgent } from '@latch/rpc/v1';
import { afterEach, expect, it } from 'vite-plus/test';
import { setTransport } from '~/lib/api/client';
import { setupDomainResponse } from '~/test/helpers';
afterEach(() => setTransport(null));
import {
  emptyEntryInput,
  entryToInput,
  formatBytes,
  redactPayload,
  mergeKeepOnNone
} from './entries';

it('forwards redaction to Rust and returns only the Rust result', async () => {
  const input = emptyEntryInput('profile', EntryType.SECRET).payload!;
  const output = { payload: input, redacted: ['value'] };
  const call = setupDomainResponse({ operation: { case: 'redactPayload', value: output } });
  expect(await redactPayload(input)).toMatchObject(output);
  expect(call).toHaveBeenCalledWith(
    LatchAgent.method.domainCall,
    expect.objectContaining({
      operation: expect.objectContaining({ case: 'redactPayload', value: input })
    })
  );
});
it('forwards both merge payloads without a client fallback', async () => {
  const stored = create(EntryPayloadSchema, {
    data: { case: 'secret', value: { value: 'fixture' } }
  });
  const input = create(EntryPayloadSchema, { data: { case: 'secret', value: {} } });
  const call = setupDomainResponse({ operation: { case: 'mergeKeepOnNone', value: stored } });
  expect(await mergeKeepOnNone(stored, input)).toEqual(stored);
  expect(call).toHaveBeenCalledWith(
    LatchAgent.method.domainCall,
    expect.objectContaining({
      operation: expect.objectContaining({
        case: 'mergeKeepOnNone',
        value: expect.objectContaining({ stored, input })
      })
    })
  );
});
it('copies form data without sharing mutable tags or payload', () => {
  const entry = emptyEntryInput('p', EntryType.SECRET);
  const input = entryToInput(entry);
  expect(input).toEqual(entry);
  expect(input.tags).not.toBe(entry.tags);
  expect(input.payload).not.toBe(entry.payload);
});
it('formats file sizes for presentation', () => {
  expect(formatBytes(6)).toBe('6 B');
  expect(formatBytes(1536)).toBe('1.5 KB');
  expect(formatBytes(1048576)).toBe('1.0 MB');
});
