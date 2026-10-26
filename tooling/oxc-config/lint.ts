import type { UserConfig } from 'vite-plus';

// Portable correctness is shared by the extension, desktop, web and developer tools.
// TypeScript's workspace typecheck owns type resolution, including generated wire types.
export const lint = {
  plugins: [
    'eslint',
    'typescript',
    'unicorn',
    'oxc',
    'react',
    'jsx-a11y',
    'import',
    'node',
    'vitest'
  ],
  categories: { correctness: 'error' },
  ignorePatterns: [
    // The landing page is owned and validated in its concurrent thread.
    'apps/landing/**',
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
    '**/.cache/**',
    '.dev/**',
    '.agents/**',
    '.impeccable/**',
    '.claude/**',
    '.codex/**'
  ],
  rules: {
    'eslint/no-unused-vars': [
      'error',
      { args: 'all', argsIgnorePattern: '^_', caughtErrors: 'none', varsIgnorePattern: '^_' }
    ],
    'eslint/prefer-const': 'error',
    'react/rules-of-hooks': 'error',
    // Mock implementations already infer their signatures; explicit generics are optional.
    'vitest/require-mock-type-parameters': 'off',
    // Vitest supports an optional assertion message (unlike Jest).
    'vitest/valid-expect': ['error', { maxArgs: 2 }],
    // Base UI controls render native inputs, including when nested inside a label.
    'jsx-a11y/label-has-associated-control': [
      'error',
      { controlComponents: ['Checkbox', 'Switch', 'Radio'] }
    ],
    // ARIA permits custom visual meters, status indicators and grouped controls.
    // Keep role validity and keyboard interaction checks; do not force a different DOM.
    'jsx-a11y/prefer-tag-over-role': 'off',
    // Autofocus belongs to Base UI's dialog/command focus management, not raw DOM inputs.
    'jsx-a11y/no-autofocus': ['error', { ignoreNonDOM: true }],
    'react/exhaustive-deps': 'error',
    'typescript/no-explicit-any': 'error',
    'typescript/consistent-type-imports': ['error', { disallowTypeAnnotations: false }],
    'typescript/consistent-type-exports': 'error',
    'vitest/no-focused-tests': 'error',
    'vitest/no-disabled-tests': 'error',
    'unicorn/prefer-node-protocol': 'error'
  },
  overrides: [
    {
      // coss-ui keeps list semantics for VoiceOver when CSS removes list markers.
      files: ['packages/ui/src/components/wizard.tsx'],
      rules: { 'jsx-a11y/no-redundant-roles': 'off' }
    }
  ]
} satisfies NonNullable<UserConfig['lint']>;
