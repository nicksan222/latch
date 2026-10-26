import { afterEach, describe, expect, it } from 'vite-plus/test';
import { applyTheme, resolveTheme, useThemeStore } from './theme';
import { useUiStore } from './ui';

afterEach(() => {
  document.documentElement.classList.remove('dark');
  localStorage.clear();
});

describe('ui store', () => {
  it('tracks palette, onboarding and per-profile type filters', () => {
    const s = useUiStore.getState();
    s.setPaletteOpen(true);
    s.setTypeFilter('p1', 'login');
    s.setTypeFilter('p2', null);
    expect(useUiStore.getState()).toMatchObject({
      paletteOpen: true,
      typeFilter: { p1: 'login', p2: null }
    });
    s.setAddingVault(true);
    expect(useUiStore.getState().addingVault).toBe(true);
  });
});

describe('theme store', () => {
  it('resolves system against the OS preference', () => {
    expect(resolveTheme('system', true)).toBe('dark');
    expect(resolveTheme('system', false)).toBe('light');
    expect(resolveTheme('light', true)).toBe('light');
  });
  it('applies the coss .dark class and persists the choice', () => {
    useThemeStore.getState().setTheme('dark');
    expect(document.documentElement.classList.contains('dark')).toBe(true);
    expect(localStorage.getItem('latch:theme')).toBe('dark');
    useThemeStore.getState().setTheme('light');
    expect(document.documentElement.classList.contains('dark')).toBe(false);
    applyTheme('dark');
    expect(document.documentElement.classList.contains('dark')).toBe(true);
  });
});
