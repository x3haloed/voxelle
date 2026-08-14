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
    --latest \
    --notes "Manually built beta release. Verify VOXELLE-RELEASE.json before installing or activating any package."
fi

latest_tag=$(gh release view --json tagName --jq .tagName)
is_prerelease=$(gh release view "$tag" --json isPrerelease --jq .isPrerelease)
if [ "$latest_tag" != "$tag" ] || [ "$is_prerelease" != false ]; then
  printf '%s\n' \
    'release must be latest and must not use the GitHub pre-release flag; the signed manifest carries the beta channel' >&2
  exit 1
fi

gh release view "$tag" --json tagName,url,isPrerelease,assets
