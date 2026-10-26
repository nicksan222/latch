#!/usr/bin/env bash
# Source this file to keep all native development operations away from real vaults.
LATCH_DEV_REPO=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
export LATCH_CONFIG_DIR="$LATCH_DEV_REPO/.dev/latch/config"
export LATCH_DATA_DIR="$LATCH_DEV_REPO/.dev/latch/data"
export LATCH_RUNTIME_DIR="$LATCH_DEV_REPO/.dev/latch/runtime"
LATCH_DEV_ID=$(printf %s "$LATCH_DEV_REPO" | sha256sum | cut -c1-12)
LATCH_RAM_DIR="/dev/shm/latch-dev-$(id -u)-$LATCH_DEV_ID"
export LATCH_RAM_DIR
unset LATCH_ALLOW_WEAK_KDF
