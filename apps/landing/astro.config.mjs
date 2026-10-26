import { fileURLToPath } from 'node:url';
import { defineConfig } from 'astro/config';
import react from '@astrojs/react';
import tailwind from '@tailwindcss/vite';
import { resolveSite } from './src/lib/site.mjs';
const { site, base } = resolveSite();
export default defineConfig({
  site, base, integrations: [react()], output: 'static', trailingSlash: 'always',
  markdown: { shikiConfig: { themes: { light: 'github-light', dark: 'github-dark' }, defaultColor: false } },
  vite: { resolve: { alias: { '~': fileURLToPath(new URL('../../apps/desktop/src', import.meta.url)) }, dedupe: ['react', 'react-dom'] }, plugins: [tailwind()], server: { fs: { allow: ['../..'] } } },
});
