import { describe, expect, it } from 'vite-plus/test';
import { defaultBrowserSettings } from './profiles';

describe('defaultBrowserSettings', () => {
  it('supplies isolated browser settings without provider controls', () => {
    const settings = defaultBrowserSettings();
    expect(settings.startPages).toEqual([]);
    expect(settings.bookmarks).toEqual([]);
    expect(settings).not.toHaveProperty('binary');
  });
});
