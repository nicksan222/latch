import { fileURLToPath, URL } from 'node:url';
import tailwindcss from '@tailwindcss/vite';
import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite-plus';

export const appSource = fileURLToPath(new URL('../../apps/desktop/src', import.meta.url));
export function frontendConfig(port: number) {
  return defineConfig({
    plugins: [react(), tailwindcss()],
    resolve: { alias: { '~': appSource }, dedupe: ['react', 'react-dom'] },
    clearScreen: false,
    server: {
      port,
      strictPort: true,
      host: process.env.TAURI_DEV_HOST || '127.0.0.1',
      proxy: {
        '/__latch': { target: process.env.LATCH_WEB_BRIDGE_URL ?? 'http://127.0.0.1:1430' }
      },
      fs: { allow: [fileURLToPath(new URL('../..', import.meta.url))] }
    },
    build: { target: 'es2022', sourcemap: false, chunkSizeWarningLimit: 1200 }
  });
}
