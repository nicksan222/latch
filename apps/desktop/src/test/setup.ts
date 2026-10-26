import '@testing-library/jest-dom/vitest';
import { cleanup } from '@testing-library/react';
import { afterEach } from 'vite-plus/test';

// Base UI / layout code paths that jsdom lacks.
if (typeof window !== 'undefined') {
  // jsdom exposes scrollTo but only logs a not-implemented error; units have no viewport.
  window.scrollTo = () => {};
  window.matchMedia ??= ((query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addEventListener: () => {},
    removeEventListener: () => {},
    addListener: () => {},
    removeListener: () => {},
    dispatchEvent: () => false
  })) as typeof window.matchMedia;
  globalThis.ResizeObserver ??= class {
    observe() {}
    unobserve() {}
    disconnect() {}
  } as unknown as typeof ResizeObserver;
  Element.prototype.scrollIntoView ??= function scrollIntoView() {};
  Element.prototype.getAnimations ??= () => [];
}

afterEach(() => {
  cleanup();
});
