import { test } from 'node:test';
import assert from 'node:assert/strict';
import { documentLinks } from './html-links.mjs';
test('React style resource names are not external assets; navigation and scripts are checked', () => {
  const html = '<style href="base-ui-disable-scrollbar" precedence="medium">body{}</style><a href="https://github.com/nicksan222/latch#readme">README</a><script src="/_astro/app.js"></script><link href="/_astro/style.css" rel="stylesheet"><div data-src="not-a-resource"></div>';
  assert.deepEqual(documentLinks(html), [['href', 'https://github.com/nicksan222/latch#readme'], ['src', '/_astro/app.js'], ['href', '/_astro/style.css']]);
});
