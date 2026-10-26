import { Theme } from '@latch/rpc/v1';
import { create } from 'zustand';

const STORAGE_KEY = 'latch:theme';

function readStoredTheme(): Theme {
  try {
    const v = localStorage.getItem(STORAGE_KEY);
    if (v === 'light' || v === 'dark' || v === 'system')
      return v === 'light' ? Theme.LIGHT : v === 'dark' ? Theme.DARK : Theme.SYSTEM;
  } catch {
    // ignore
  }
  return Theme.SYSTEM;
}

export function systemPrefersDark(): boolean {
  return (
    typeof window !== 'undefined' &&
    window.matchMedia?.('(prefers-color-scheme: dark)').matches === true
  );
}

export function resolveTheme(theme: Theme, prefersDark = systemPrefersDark()): 'light' | 'dark' {
  return theme === Theme.LIGHT
    ? 'light'
    : theme === Theme.DARK
      ? 'dark'
      : prefersDark
        ? 'dark'
        : 'light';
}

/** Applies the coss `.dark` class on <html>; transitions are suppressed for one frame. */
export function applyTheme(theme: Theme) {
  if (typeof document === 'undefined') return;
  const root = document.documentElement;
  const dark = resolveTheme(theme) === 'dark';
  if (root.classList.contains('dark') === dark) return;
  root.classList.add('no-transitions');
  root.classList.toggle('dark', dark);
  root.style.colorScheme = dark ? 'dark' : 'light';
  requestAnimationFrame(() => root.classList.remove('no-transitions'));
}

interface ThemeState {
  theme: Theme;
  setTheme: (theme: Theme) => void;
}

/** Mirrors `settings.theme` locally so the lock screen is themed before unlock. */
export const useThemeStore = create<ThemeState>((set) => ({
  theme: readStoredTheme(),
  setTheme: (theme) => {
    try {
      localStorage.setItem(
        STORAGE_KEY,
        theme === Theme.LIGHT ? 'light' : theme === Theme.DARK ? 'dark' : 'system'
      );
    } catch {
      // ignore
    }
    applyTheme(theme);
    set({ theme });
  }
}));
