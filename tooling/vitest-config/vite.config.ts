import { appSource } from '@latch/vite-config/frontend';
import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite-plus';
export default defineConfig({
  plugins: [react()],
  defaultPackage: './apps/desktop',
  resolve: { alias: { '~': appSource }, dedupe: ['react', 'react-dom'] },
  test: {
    globals: true,
    environment: 'jsdom',
    setupFiles: [
      './tooling/vitest-config/src/console-guard/setup.ts',
      './apps/desktop/src/test/setup.ts'
    ],
    // Cleanup hooks run before the console guard finishes each test.
    sequence: { hooks: 'stack' },
    include: [
      'apps/desktop/src/**/*.test.{ts,tsx}',
      'packages/**/*.test.{ts,tsx}',
      'tooling/**/*.test.{ts,tsx}'
    ]
  }
});
