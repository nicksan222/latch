#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."
bun install --frozen-lockfile
bun run hooks:install
bunx --no-install playwright install chromium
bun run dev:seed
printf '\nLatch is seeded. The desktop starts automatically; open http://127.0.0.1:5900/vnc.html?autoconnect=1&resize=scale\n'
bash tooling/development/welcome.sh
