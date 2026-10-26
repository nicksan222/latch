#!/usr/bin/env bash
# Docker's default namespace restrictions prevent Chromium's inner sandbox.
# Used only by the fictional demo desktop, never by normal Latch launches.
set -euo pipefail
exec /usr/bin/chromium --no-sandbox "$@"
