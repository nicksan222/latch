#!/usr/bin/env bash
# Run only in the disposable release build checkout, after npm ci/version prep.
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
cd "$root"
version=${1:?release version required}
node --input-type=module -e 'import { parseVersion } from "./tooling/release/release-plan.mjs"; parseVersion(process.argv[1])' "$version"
[[ "${LATCH_RELEASE_BUILD:-}" == 1 && "${CI:-}" == true ]]
triple=$(rustc -vV | sed -n 's/^host: //p')
[[ "$triple" == x86_64-unknown-linux-gnu ]]
output="$root/release-artifacts"
[[ ! -e "$output" ]] || { echo 'Refusing to reuse artifact directory' >&2; exit 1; }
mkdir -p "$output"
# Tauri runs the owned desktop frontend and CLI beforeBuildCommand.
npm run desktop:bundle -- --bundles deb,appimage -- --locked
binaries="$root/apps/desktop/src-tauri/binaries"
"$binaries/latch" --version
# Bundle only the CLI binary; never package the workspace or .dev data.
archive="latch-${version}-${triple}.tar.gz"
tar -czf "$output/$archive" -C "$binaries" latch -C "$root" LICENSE
# The Tauri build is a host build; the CLI uses its explicit target subdirectory.
target_dir=${CARGO_TARGET_DIR:-$root/target}
for kind in deb appimage; do
  found=0
  while IFS= read -r -d '' bundle; do
    cp "$bundle" "$output/"
    found=$((found + 1))
  done < <(find "$target_dir/release/bundle/$kind" -maxdepth 1 -type f \( -name '*.deb' -o -name '*.AppImage' \) -print0)
  [[ "$found" == 1 ]] || { echo "Expected one $kind bundle" >&2; exit 1; }
done
(cd "$output" && sha256sum -- * > SHA256SUMS && sha256sum --check SHA256SUMS)
