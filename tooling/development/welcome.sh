#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."
printf '%s\n' \
  'Latch developer workspace' \
  '  bunx --no-install just --list   Discover workflows (no global Just needed)' \
  '  bunx --no-install just dev      Desktop UI with real Rust fixtures' \
  '  bunx --no-install just desktop  Isolated native demo desktop' \
  '  bunx --no-install just quality  Non-mutating quality gate' \
  '  bunx --no-install just check    Complete verification' \
  '  See README.md and bun run workflows.'
node tooling/development/doctor.mjs "$@"
