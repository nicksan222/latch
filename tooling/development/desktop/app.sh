#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../../.."
# shellcheck source=tooling/development/dev-env.sh
source tooling/development/dev-env.sh
export DISPLAY=:99 GDK_BACKEND=x11 WEBKIT_DISABLE_DMABUF_RENDERER=1
export LATCH_BROWSER="$PWD/tooling/development/desktop/chromium.sh"
unset WAYLAND_DISPLAY
npm run dev:seed
cargo build --locked -p latch-desktop -p latch-cli
exec dbus-run-session -- npm run desktop:dev
