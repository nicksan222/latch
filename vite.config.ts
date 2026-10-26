import workspace from '@latch/vitest-config/workspace';
import { defineConfig } from 'vite-plus';
import { fmt } from './tooling/oxc-config/fmt';
import { lint } from './tooling/oxc-config/lint';

export default defineConfig({ ...workspace, fmt, lint });
