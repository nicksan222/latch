import { describe, expect, it } from 'vite-plus/test';
import { defaultBrowserSettings } from './profiles';

describe('defaultBrowserSettings', () => {
  it('selects managed Firefox without a host binary override', () => {
    expect(defaultBrowserSettings()).toMatchObject({
      engine: 'firefox',
      binary: null
    });
  });
});
