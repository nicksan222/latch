import '@latch/ui/styles.css';
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { App } from './app/app';
import { applyTheme, useThemeStore } from './stores/theme';

applyTheme(useThemeStore.getState().theme);

export function mountApp(element: HTMLElement) {
  createRoot(element).render(
    <StrictMode>
      <App />
    </StrictMode>
  );
}
