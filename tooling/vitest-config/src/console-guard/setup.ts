import { afterAll, afterEach, beforeEach } from 'vite-plus/test';
import { finishFile, finishTest, install, startTest } from './state';

// Catch output while the file loads, and again per test: Vitest may hand a test a new console.
install();
beforeEach(() => {
  install();
  startTest();
});
afterEach(finishTest);
afterAll(finishFile);
