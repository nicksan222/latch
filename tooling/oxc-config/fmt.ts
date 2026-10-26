import type { UserConfig } from 'vite-plus';

export const fmt = {
  endOfLine: 'lf',
  printWidth: 100,
  singleQuote: true,
  trailingComma: 'none',
  sortImports: { newlinesBetween: false, partitionByNewline: true },
  // Syncpack owns manifest ordering; Oxfmt must not rewrite its decisions.
  sortPackageJson: false,
  ignorePatterns: [
    // The landing page is owned and validated in its concurrent thread.
    'apps/landing/**',
    '**/package.json',
    '**/bun.lock',
    '**/*.{md,mdx,toml,yaml,yml}',
    '**/_generated/**',
    'packages/rpc/ts/**',
    '**/generated.ts',
    '**/node_modules/**',
    '**/dist/**',
    '**/dist-firefox/**',
    '**/build/**',
    '**/target/**',
    '**/coverage/**',
    '**/test-results/**',
    '**/playwright-report/**',
    '**/vendor/**',
    // Licensed upstream crate source: keep its original packaging metadata intact.
    'tooling/patches/**',
    '**/.cache/**',
    '.dev/**',
    '.agents/**',
    '.impeccable/**',
    '.claude/**',
    '.codex/**'
  ]
} satisfies NonNullable<UserConfig['fmt']>;
