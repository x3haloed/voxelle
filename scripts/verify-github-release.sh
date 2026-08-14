#!/bin/sh
set -eu

if [ "$#" -ne 1 ]; then
  printf 'usage: %s CLEAN_DOWNLOAD_DIR\n' "$0" >&2
  exit 2
fi

download_dir=$1
repo_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
release_base=https://github.com/x3haloed/voxelle/releases/latest/download
manifest_name=VOXELLE-RELEASE.json
trust_roots="$repo_dir/release/trusted-update-keys.json"

command -v curl >/dev/null 2>&1 || {
  printf 'curl is required for release readback.\n' >&2
  exit 1
}

if [ -e "$download_dir" ]; then
  printf 'refusing to reuse download directory: %s\n' "$download_dir" >&2
  exit 1
fi
mkdir -m 700 "$download_dir"

curl --proto '=https' --tlsv1.2 --fail --location --max-redirs 5 \
  --retry 2 --max-filesize 1048576 \
  --output "$download_dir/$manifest_name" \
  "$release_base/$manifest_name"

cargo run -q --manifest-path "$repo_dir/Cargo.toml" -p voxelle-release -- \
  list-release-artifacts \
  --trust-roots "$trust_roots" \
  --manifest "$download_dir/$manifest_name" |
while IFS= read -r asset; do
  case "$asset" in
    ''|.|..|*/*|*\\*)
      printf 'refusing unsafe signed asset name: %s\n' "$asset" >&2
      exit 1
      ;;
  esac
  curl --proto '=https' --tlsv1.2 --fail --location --max-redirs 5 \
    --retry 2 --max-filesize 2147483648 \
    --output "$download_dir/$asset" \
    "$release_base/$asset"
done

cargo run -q --manifest-path "$repo_dir/Cargo.toml" -p voxelle-release -- \
  verify-release \
  --trust-roots "$trust_roots" \
  --manifest "$download_dir/$manifest_name" \
  --artifact-dir "$download_dir"

printf 'Read back and verified the latest signed GitHub Release in %s\n' "$download_dir"
