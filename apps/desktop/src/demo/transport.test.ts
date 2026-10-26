import { createClient } from '@connectrpc/connect';
import { EntryType, ErrorCode, Theme, LatchAgent } from '@latch/rpc/v1';
import { describe, expect, it } from 'vite-plus/test';
import { connectTransport } from '../lib/api/transport';
import { createDemoTransport } from './transport';

describe('offline public demo transport', () => {
  it('serves realistic summaries, scoped filters and redacted detail responses', async () => {
    const demo = createDemoTransport();
    const transport = createClient(LatchAgent, connectTransport(demo));
    demo.setTheme(Theme.LIGHT);
    expect((await transport.settingsGet({})).theme).toBe(Theme.LIGHT);
    for (const list of [
      transport.vaultList,
      transport.entriesTrash,
      transport.recipientsList,
      transport.browserList,
      transport.totpPinned
    ]) {
      expect(await list({})).toMatchObject({ items: [] });
    }
    const profiles = (await transport.profilesList({})).items;
    const all = (await transport.entriesList({})).items;
    for (const profile of profiles) {
      expect(all.filter((entry) => entry.profileId === profile.id)).toHaveLength(
        profile.entryCount
      );
    }
    const env = (
      await transport.entriesList({
        profileId: 'acme',
        type: EntryType.ENV,
        query: 'development'
      })
    ).items;
    expect(env).toHaveLength(1);
    expect(env[0]?.subtitle).toBe('5 vars');
    const entry = await transport.entriesGet({ id: env[0]!.id });
    expect(entry.payload).toMatchObject({
      data: {
        case: 'env',
        value: {
          vars: expect.arrayContaining([
            expect.objectContaining({
              key: 'DATABASE_URL',
              hidden: true,
              comment: 'Development database'
            })
          ])
        }
      }
    });
    const data = entry.payload?.data;
    expect(
      data?.case === 'env' && data.value.vars.find((item) => item.key === 'DATABASE_URL')?.value
    ).toBeUndefined();
    expect((await transport.entriesHistory({ id: entry.id })).items[0]?.changedFields).toEqual([
      'password'
    ]);
    await expect(transport.entriesGet({ id: entry.id, reveal: true })).rejects.toMatchObject({
      code: ErrorCode.INVALID_PARAMS
    });
    // A caller cannot modify the fixture by mutating a returned entry.
    entry.name = 'Changed';
    expect(await transport.entriesGet({ id: entry.id })).not.toMatchObject({
      name: 'Changed'
    });
  });

  it('reveals only requested fictional fields and blocks reads while locked', async () => {
    const demo = createDemoTransport();
    const transport = createClient(LatchAgent, connectTransport(demo));
    expect(await transport.entriesReveal({ id: 'acme-login', field: 'password' })).toMatchObject({
      value: 'demo-only-acme-password'
    });
    expect(await transport.vaultLock({})).toMatchObject({ locked: true });
    expect(await transport.vaultStatus({})).toMatchObject({ locked: true });
    await expect(transport.entriesGet({ id: 'acme-login' })).rejects.toMatchObject({
      code: ErrorCode.LOCKED
    });
    expect(await transport.vaultUnlock({})).toMatchObject({ locked: false });
    expect(await transport.entriesGet({ id: 'acme-login' })).toMatchObject({
      payload: { data: { case: 'login', value: {} } }
    });
    await expect(transport.entriesUpdate({})).rejects.toThrow('read-only fictional data');
  });
});
