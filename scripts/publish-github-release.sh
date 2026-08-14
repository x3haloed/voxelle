#!/bin/sh
set -eu

if [ "$#" -lt 3 ]; then
  printf 'usage: %s TAG TITLE ASSET...\n' "$0" >&2
  exit 2
fi

tag=$1
title=$2
shift 2

command -v gh >/dev/null 2>&1 || {
  printf 'GitHub CLI (gh) is required for manual publication.\n' >&2
  exit 1
}
gh auth status >/dev/null

if gh release view "$tag" >/dev/null 2>&1; then
  gh release upload "$tag" "$@"
else
  gh release create "$tag" "$@" \
    --title "$title" \
    --notes "Manually built beta release. Verify VOXELLE-RELEASE.json before installing or activating any package."
fi

gh release view "$tag" --json tagName,url,assets
