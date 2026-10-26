import { defineConfig, devices } from '@playwright/test';
import { selectE2ERun } from './prerequisites';

const run = selectE2ERun();
const url = `http://127.0.0.1:${run.port}`;

/** Desktop UI flows against isolated Rust fixture agents on a dedicated test server. */
export default defineConfig({
  testDir: '../../apps/desktop/tests/e2e',
  testMatch: '**/*.spec.ts',
  outputDir: `../../.dev/test-results/run-${run.id}`,
  fullyParallel: true,
  // Keep Chromium renderer memory bounded in the development container.
  workers: process.env.CI ? 2 : 4,
  forbidOnly: Boolean(process.env.CI),
  retries: process.env.CI ? 1 : 0,
  reporter: [['list']],
  timeout: 30_000,
  expect: { timeout: 7_000 },
  use: {
    baseURL: url,
    trace: 'retain-on-failure',
    permissions: ['clipboard-read', 'clipboard-write'],
    viewport: { width: 1360, height: 860 }
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'], viewport: { width: 1360, height: 860 } }
    }
  ],
  webServer: {
    command: `node tooling/development/frontend.mjs --port ${run.port}`,
    cwd: new URL('../..', import.meta.url).pathname,
    url,
    reuseExistingServer: false,
    // Let the launcher release its separately owned Rust/Vite process groups.
    gracefulShutdown: { signal: 'SIGTERM', timeout: 15_000 },
    timeout: 300_000
  }
});
