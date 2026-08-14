#!/bin/sh
set -eu

if [ "$#" -lt 4 ]; then
  printf 'usage: %s TAG TITLE NOTES_FILE ASSET...\n' "$0" >&2
  exit 2
fi

tag=$1
title=$2
notes_file=$3
shift 3

repo_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
repo=x3haloed/voxelle
api=https://api.github.com
target=$(git -C "$repo_dir" rev-parse HEAD)
response_dir=$(mktemp -d /tmp/voxelle-github-release.XXXXXX)

command -v curl >/dev/null 2>&1 || {
  printf 'curl is required for GitHub API publication.\n' >&2
  exit 1
}
command -v jq >/dev/null 2>&1 || {
  printf 'jq is required for GitHub API publication.\n' >&2
  exit 1
}
test -f "$notes_file"

credential=$(printf 'protocol=https\nhost=github.com\n\n' | git credential fill)
github_user=$(printf '%s\n' "$credential" | sed -n 's/^username=//p')
github_token=$(printf '%s\n' "$credential" | sed -n 's/^password=//p')
test -n "$github_user" && test -n "$github_token"

request() {
  curl --proto '=https' --tlsv1.2 --silent --show-error \
    --user "$github_user:$github_token" \
    -H 'Accept: application/vnd.github+json' \
    -H 'X-GitHub-Api-Version: 2022-11-28' \
    "$@"
}

jq -n \
  --arg tag "$tag" \
  --arg target "$target" \
  --arg name "$title" \
  --rawfile body "$notes_file" \
  '{tag_name:$tag,target_commitish:$target,name:$name,body:$body,draft:true,prerelease:false,make_latest:"true"}' \
  > "$response_dir/create-payload.json"

create_code=$(request \
  -H 'Content-Type: application/json' \
  --data-binary @"$response_dir/create-payload.json" \
  --output "$response_dir/create-response.json" \
  --write-out '%{http_code}' \
  "$api/repos/$repo/releases")
if [ "$create_code" != 201 ]; then
  jq '{message,errors}' "$response_dir/create-response.json" >&2
  exit 1
fi

release_id=$(jq -r .id "$response_dir/create-response.json")
upload_url=$(jq -r '.upload_url | sub("\\{\\?name,label\\}"; "")' "$response_dir/create-response.json")
printf 'created draft release %s; diagnostic responses: %s\n' "$release_id" "$response_dir"

for asset in "$@"; do
  test -f "$asset"
  asset_name=$(basename "$asset")
  encoded_name=$(printf '%s' "$asset_name" | jq -sRr @uri)
  case "$asset_name" in
    *.json) content_type=application/json ;;
    *.dmg) content_type=application/x-apple-diskimage ;;
    *) content_type=application/octet-stream ;;
  esac
  upload_code=$(request \
    -H "Content-Type: $content_type" \
    --data-binary @"$asset" \
    --output "$response_dir/upload-response.json" \
    --write-out '%{http_code}' \
    "$upload_url?name=$encoded_name")
  if [ "$upload_code" != 201 ]; then
    jq '{message,errors}' "$response_dir/upload-response.json" >&2
    printf 'draft release %s remains unpublished for inspection\n' "$release_id" >&2
    exit 1
  fi
  printf 'uploaded %s (%s bytes)\n' \
    "$asset_name" \
    "$(jq -r .size "$response_dir/upload-response.json")"
done

publish_code=$(request \
  -X PATCH \
  -H 'Content-Type: application/json' \
  --data '{"draft":false,"prerelease":false,"make_latest":"true"}' \
  --output "$response_dir/publish-response.json" \
  --write-out '%{http_code}' \
  "$api/repos/$repo/releases/$release_id")
if [ "$publish_code" != 200 ]; then
  jq '{message,errors}' "$response_dir/publish-response.json" >&2
  printf 'draft release %s remains unpublished for inspection\n' "$release_id" >&2
  exit 1
fi

latest_tag=$(request "$api/repos/$repo/releases/latest" | jq -r .tag_name)
if [ "$latest_tag" != "$tag" ]; then
  printf 'published release is not latest: expected %s, got %s\n' "$tag" "$latest_tag" >&2
  exit 1
fi

jq '{tag_name,name,html_url,draft,prerelease,published_at,assets:[.assets[]|{name,size,browser_download_url}]}' \
  "$response_dir/publish-response.json"
