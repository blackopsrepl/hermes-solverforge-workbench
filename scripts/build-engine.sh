#!/usr/bin/env bash
# Build one release binary of the engine for a Rust target triple and write it to
# dist/ under its release asset name: cross-compiled from Linux with cargo-zigbuild,
# or with plain cargo on macOS, whose SDK the Apple targets link against.
# CI runs exactly this script; run it locally to check a target before tagging.
#
#   scripts/build-engine.sh x86_64-unknown-linux-musl
set -euo pipefail

target=${1:?usage: build-engine.sh <rust-target-triple>}
root=$(cd "$(dirname "$0")/.." && pwd)
dist="$root/dist"
exe=""
[[ $target == *windows* ]] && exe=".exe"

rustup target add "$target" >/dev/null
cd "$root/engine"
if [[ $(uname -s) == Darwin ]]; then
  cargo build --release --locked --target "$target" -p solverforge-workbench
else
  cargo zigbuild --release --locked --target "$target" -p solverforge-workbench
fi

mkdir -p "$dist"
asset="solverforge-workbench-$target$exe"
cp "target/$target/release/solverforge-workbench$exe" "$dist/$asset"
echo "$dist/$asset"
