import { readdir, readFile, stat } from 'node:fs/promises';
import { resolve, relative } from 'node:path';
import assert from 'node:assert/strict';
import { resolveSite, withBase } from '../src/lib/site.mjs';
import { documentLinks } from '../src/lib/html-links.mjs';
const root = resolve(import.meta.dirname, '../dist');
const { site, base } = resolveSite();
const errors = [];
async function files(dir) {
  const entries = await readdir(dir, { withFileTypes: true });
  return (await Promise.all(entries.map((entry) => entry.isDirectory() ? files(resolve(dir, entry.name)) : resolve(dir, entry.name)))).flat();
}
const all = await files(root);
const pages = all.filter((name) => name.endsWith('.html'));
const ids = new Map();
for (const path of pages) { const html = await readFile(path, 'utf8'); ids.set(path, new Set([...html.matchAll(/\bid="([^"]+)"/g)].map((match) => match[1]))); }
for (const path of pages) {
  const html = await readFile(path, 'utf8');
  const current = new URL(withBase(base, relative(root, path).replace(/index\.html$/, '')), site);
  for (const [attribute, raw] of documentLinks(html)) {
    if (/^(https?:|mailto:|data:|tel:|javascript:)/.test(raw)) continue;
    const url = new URL(raw.replace(/&amp;/g, '&'), current);
    const prefix = base === '/' ? '/' : `${base.replace(/\/$/, '')}/`;
    if (!url.pathname.startsWith(prefix)) { errors.push(`${path}: escaped base: ${raw}`); continue; }
    let target = resolve(root, decodeURIComponent(url.pathname.slice(prefix.length)));
    try { if ((await stat(target)).isDirectory()) target = resolve(target, 'index.html'); await stat(target); }
    catch { errors.push(`${relative(root, path)}: missing ${attribute} ${raw}`); continue; }
    if (url.hash && ids.has(target) && !ids.get(target).has(decodeURIComponent(url.hash.slice(1)))) errors.push(`${relative(root, path)}: missing anchor ${raw}`);
  }
}
const home = await readFile(resolve(root, 'index.html'), 'utf8');
assert.match(home, /releases\/latest/, 'Latest-build destination remains present');
assert.match(home, /https:\/\/github\.com\/nicksan222\/latch#readme/, 'Documentation links point to the project README');
assert.doesNotMatch(home, /href="[^"]*\/docs\//, 'Landing page has no local documentation routes');
assert.equal(all.some((path) => relative(root, path).startsWith('docs/')), false, 'Standalone documentation pages are not generated');
assert.equal(all.some((path) => relative(root, path) === 'search-index.json'), false, 'Documentation search index is not generated');
assert.match(await readFile(resolve(root, 'sitemap.xml'), 'utf8'), /<urlset/);
assert.equal(errors.length, 0, errors.join('\n'));
console.log(`Verified ${pages.length} HTML pages, internal links, fragments, assets, README links, release link and sitemap (base ${base}).`);
