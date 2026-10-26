#!/usr/bin/env bash
# Run the seeded app on an existing desktop (outside or inside the container).
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."
# shellcheck source=tooling/development/dev-env.sh
source tooling/development/dev-env.sh
bun run dev:seed
cargo build --locked -p latch-cli
exec bun run desktop:dev
