import { expect, it } from 'vite-plus/test';
import { totpSecondsLeft } from './totp';
it('presents the countdown across rotation boundaries', () => {
  expect(totpSecondsLeft(30, 59000)).toBe(1);
  expect(totpSecondsLeft(30, 60000)).toBe(30);
  expect(totpSecondsLeft(30, 61000)).toBe(29);
  expect(totpSecondsLeft(60, 61000)).toBe(59);
});
