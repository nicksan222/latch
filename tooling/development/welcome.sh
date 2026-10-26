#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."
printf '%s\n' \
  'Latch developer workspace' \
  '  npm exec just -- --list   Discover workflows (no global Just needed)' \
  '  npm exec just -- dev      Desktop UI with real Rust fixtures' \
  '  npm exec just -- desktop  Isolated native demo desktop' \
  '  npm exec just -- quality  Non-mutating quality gate' \
  '  npm exec just -- check    Complete verification' \
  '  See README.md and npm run workflows.'
node tooling/development/doctor.mjs "$@"
