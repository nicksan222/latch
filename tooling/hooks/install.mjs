#!/usr/bin/env node
import { execFileSync } from 'node:child_process';
import { assertHookPath } from './policy.mjs';

const git = (...args) => execFileSync('git', args, { encoding: 'utf8' }).trim();
try {
  git('rev-parse', '--show-toplevel'); // Works before the first commit.
  let current = '';
  try {
    current = git('config', '--get', 'core.hooksPath');
  } catch (error) {
    if (error.status !== 1) throw error;
  }
  git('config', '--local', 'core.hooksPath', assertHookPath(current));
  console.log('Local hooks enabled: tooling/hooks (npm run precommit, checks only).');
} catch (error) {
  console.error(error.message);
  process.exitCode = 1;
}
