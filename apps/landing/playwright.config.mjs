import { defineConfig } from '@playwright/test';
const port = Number(process.env.LANDING_TEST_PORT || 4333);
const outputDir = process.env.LANDING_TEST_OUTPUT || '../../.dev/test-results/landing';
export default defineConfig({
  testDir: './tests', fullyParallel: false, workers: 1,
  reporter: 'list', outputDir,
  use: { baseURL: `http://127.0.0.1:${port}`, headless: true, trace: 'retain-on-failure' },
  webServer: { command: `python3 -m http.server ${port} --bind 127.0.0.1 --directory dist`, url: `http://127.0.0.1:${port}`, reuseExistingServer: !process.env.CI }
});
