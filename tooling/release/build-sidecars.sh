#!/bin/sh
# Builds only the CLI for desktop Linux packaging. Tauri's bundle config places
# it in /usr/bin/latch in deb/rpm; it is also shipped as its own release archive.
# Kept at this path for the existing workspace sidecars command.
set -eu
root=$(cd "$(dirname "$0")/../.." && pwd)
manifest="$root/Cargo.toml"
triple=${TAURI_ENV_TARGET_TRIPLE:-$(rustc -vV | sed -n 's/^host: //p')}
target_dir=${CARGO_TARGET_DIR:-$root/target}
case "$target_dir" in /*) ;; *) target_dir="$root/$target_dir" ;; esac
CARGO_TARGET_DIR=$target_dir
export CARGO_TARGET_DIR
if [ "${TAURI_ENV_DEBUG:-false}" = "true" ]; then
  profile=debug; flag=""
else
  profile=release; flag="--release"
fi
cargo build --locked --manifest-path "$manifest" --no-default-features -p latch-cli $flag \
  --target "$triple" --bin latch
mkdir -p "$root/apps/desktop/src-tauri/binaries"
ext=""
case "$triple" in *windows*) ext=".exe" ;; esac
cp "$target_dir/$triple/$profile/latch$ext" "$root/apps/desktop/src-tauri/binaries/latch$ext"
echo "CLI ready for $triple ($profile)"
