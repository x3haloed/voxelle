#!/bin/sh
set -eu

if [ "$#" -lt 4 ]; then
  printf 'usage: %s RELEASE_ID SEQUENCE OUTPUT_MANIFEST ARTIFACT...\n' "$0" >&2
  exit 2
fi

release_id=$1
sequence=$2
output=$3
shift 3
repo_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
release_key=${VOXELLE_RELEASE_KEY:-"$HOME/.config/voxelle-release/signing-key.json"}
first_artifact=$1

artifact_args=
for artifact in "$@"; do
  artifact_args="$artifact_args --artifact $artifact"
done

# Packaging outputs have whitespace-free paths; expand one explicit clap option
# per artifact.
# shellcheck disable=SC2086
cargo run -q -p voxelle-release -- sign-manifest \
  --secret "$release_key" \
  --output "$output" \
  --release-id "$release_id" \
  --sequence "$sequence" \
  --channel beta \
  $artifact_args

artifact_dir=$(CDPATH= cd -- "$(dirname -- "$first_artifact")" && pwd)
cargo run -q -p voxelle-release -- verify-release \
  --trust-roots "$repo_dir/release/trusted-update-keys.json" \
  --manifest "$output" \
  --artifact-dir "$artifact_dir"
printf 'Signed and verified release manifest %s\n' "$output"

