#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../../.."
# shellcheck source=tooling/development/dev-env.sh
source tooling/development/dev-env.sh
export DISPLAY=:99 GDK_BACKEND=x11 WEBKIT_DISABLE_DMABUF_RENDERER=1
unset WAYLAND_DISPLAY
bun run dev:seed
cargo build --locked -p latch-desktop -p latch-cli
exec dbus-run-session -- bun run desktop:dev
