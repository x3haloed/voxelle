#!/bin/sh
set -eu

repo_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repo_dir/crates/voxelle-tauri-host"

if rustup target list --installed | grep -qx 'aarch64-apple-darwin' \
  && rustup target list --installed | grep -qx 'x86_64-apple-darwin'; then
  cargo tauri build --target universal-apple-darwin --bundles dmg
  bundle_dir="$repo_dir/target/universal-apple-darwin/release/bundle/dmg"
else
  cargo tauri build --bundles dmg
  bundle_dir="$repo_dir/target/release/bundle/dmg"
fi

(cd "$bundle_dir" && shasum -a 256 ./*.dmg > SHA256SUMS.txt)
printf 'Built unsigned macOS release in %s\n' "$bundle_dir"
