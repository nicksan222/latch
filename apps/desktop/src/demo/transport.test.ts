import { describe, expect, it } from 'vite-plus/test';
import type { Entry, EntrySummary, ProfileSummary } from '../_generated/contracts';
import { createDemoTransport } from './transport';

describe('offline public demo transport', () => {
  it('serves realistic summaries, scoped filters and redacted detail responses', async () => {
    const transport = createDemoTransport();
    const profiles = (await transport.call('profiles.list', {})) as ProfileSummary[];
    const all = (await transport.call('entries.list', {})) as EntrySummary[];
    for (const profile of profiles) {
      expect(all.filter((entry) => entry.profileId === profile.id)).toHaveLength(
        profile.entryCount
      );
    }
    const env = (await transport.call('entries.list', {
      profileId: 'acme',
      type: 'env',
      query: 'development'
    })) as EntrySummary[];
    expect(env).toHaveLength(1);
    expect(env[0]?.subtitle).toBe('5 vars');
    const entry = (await transport.call('entries.get', { id: env[0]!.id })) as Entry;
    expect(entry.payload).toMatchObject({
      data: {
        vars: expect.arrayContaining([
          { key: 'DATABASE_URL', value: null, hidden: true, comment: 'Development database' }
        ])
      }
    });
    // A caller cannot modify the fixture by mutating a returned entry.
    entry.name = 'Changed';
    expect(await transport.call('entries.get', { id: entry.id })).not.toMatchObject({
      name: 'Changed'
    });
  });

  it('reveals only requested fictional fields and blocks reads while locked', async () => {
    const transport = createDemoTransport();
    expect(await transport.call('entries.reveal', { id: 'acme-login', field: 'password' })).toEqual(
      { value: 'demo-only-acme-password' }
    );
    await transport.call('vault.lock', {});
    await expect(transport.call('entries.get', { id: 'acme-login' })).rejects.toMatchObject({
      code: 'locked'
    });
    await transport.call('vault.unlock', {});
    expect(await transport.call('entries.get', { id: 'acme-login' })).toMatchObject({
      payload: { data: { password: null } }
    });
    await expect(transport.call('entries.update', {})).rejects.toThrow('read-only fictional data');
  });
});
