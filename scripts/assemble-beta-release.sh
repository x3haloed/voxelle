#!/bin/sh
set -eu

if [ "$#" -lt 5 ] || [ "$#" -gt 6 ]; then
  printf 'usage: %s RELEASE_ID SEQUENCE OUTPUT_DIR MACOS_DMG WINDOWS_EXE [GENERATION_JSON]\n' "$0" >&2
  exit 2
fi

release_id=$1
sequence=$2
output_dir=$3
macos_dmg=$4
windows_exe=$5
generation_json=${6:-}
repo_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)

case "$sequence" in
  ''|*[!0-9]*)
    printf 'sequence must be an unsigned integer\n' >&2
    exit 2
    ;;
esac

for artifact in "$macos_dmg" "$windows_exe"; do
  if [ ! -f "$artifact" ]; then
    printf 'required native artifact is missing: %s\n' "$artifact" >&2
    exit 1
  fi
done
case "$macos_dmg" in
  *.dmg) ;;
  *) printf 'macOS artifact must end in .dmg: %s\n' "$macos_dmg" >&2; exit 1 ;;
esac
case "$windows_exe" in
  *.exe) ;;
  *) printf 'Windows artifact must end in .exe: %s\n' "$windows_exe" >&2; exit 1 ;;
esac

if [ -e "$output_dir" ]; then
  printf 'refusing to reuse release output directory: %s\n' "$output_dir" >&2
  exit 1
fi

"$repo_dir/scripts/verify-beta-source.sh"

mkdir -m 700 "$output_dir"

macos_output="$output_dir/$(basename -- "$macos_dmg")"
windows_output="$output_dir/$(basename -- "$windows_exe")"
if [ "$macos_output" = "$windows_output" ]; then
  printf 'native artifact names collide: %s\n' "$(basename -- "$macos_dmg")" >&2
  exit 1
fi
cp "$macos_dmg" "$macos_output"
cp "$windows_exe" "$windows_output"

if [ -n "$generation_json" ]; then
  "$repo_dir/scripts/prepare-product-update.sh" \
    "$release_id" "$sequence" "$output_dir" "$generation_json"
else
  "$repo_dir/scripts/prepare-product-update.sh" \
    "$release_id" "$sequence" "$output_dir"
fi

product_update="$output_dir/$release_id.voxupdate"
manifest="$output_dir/VOXELLE-RELEASE.json"
"$repo_dir/scripts/sign-release.sh" \
  "$release_id" "$sequence" "$manifest" \
  "$macos_output" "$windows_output" "$product_update"

(cd "$output_dir" && shasum -a 256 \
  "$(basename -- "$macos_output")" \
  "$(basename -- "$windows_output")" \
  "$(basename -- "$product_update")" \
  VOXELLE-RELEASE.json > SHA256SUMS.txt)

printf 'Assembled signed beta release %s sequence %s in %s\n' \
  "$release_id" "$sequence" "$output_dir"
printf 'Native installation and first launch must still be exercised on macOS and Windows.\n'
