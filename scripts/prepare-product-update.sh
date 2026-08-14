#!/bin/sh
set -eu

if [ "$#" -lt 3 ] || [ "$#" -gt 4 ]; then
  printf 'usage: %s RELEASE_ID SEQUENCE OUTPUT_DIR [GENERATION_JSON]\n' "$0" >&2
  exit 2
fi

release_id=$1
sequence=$2
output_dir=$3
generation_json=${4:-}
repo_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
release_key=${VOXELLE_RELEASE_KEY:-"$HOME/.config/voxelle-release/signing-key.json"}

mkdir -p "$output_dir"
cd "$repo_dir"
if [ -z "$generation_json" ]; then
  generation_json="$output_dir/$release_id.generation.json"
  cargo run -q -p voxelle-release -- generation-template --output "$generation_json"
fi

output="$output_dir/$release_id.voxupdate"
cargo run -q -p voxelle-release -- package-generation \
  --secret "$release_key" \
  --generation "$generation_json" \
  --output "$output" \
  --release-id "$release_id" \
  --sequence "$sequence" \
  --channel beta \
  --min-kernel-version 0.1.0
cargo run -q -p voxelle-release -- verify-package \
  --trust-roots "$repo_dir/release/trusted-update-keys.json" \
  --package "$output" \
  --kernel-version 0.1.0
printf 'Prepared verified product update %s\n' "$output"

