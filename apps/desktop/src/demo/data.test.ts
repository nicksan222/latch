import { describe, expect, it } from 'vite-plus/test';
import { demoEntries, demoProfiles, demoValue } from './data';

describe('public landing demo data', () => {
  it('keeps projects distinct and secret payloads redacted until requested', () => {
    const entries = demoProfiles.flatMap((profile) => demoEntries(profile.id));
    expect(new Set(entries.map((entry) => entry.id)).size).toBe(entries.length);
    expect(demoEntries('acme')[0]?.payload).toMatchObject({ data: { case: 'login', value: {} } });
    const login = demoEntries('acme')[0]?.payload?.data;
    expect(login?.case === 'login' && login.value.password).toBeUndefined();
    expect(demoValue('personal-login', 'password')).toBe('demo-only-personal-password');
    expect(() => demoValue('missing', 'password')).toThrow('Demo entry not found');
    expect(() => demoValue('acme-login', 'arbitrary')).toThrow('Demo field not found');
  });
});
