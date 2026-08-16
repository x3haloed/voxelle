#!/bin/sh
set -eu

repo_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repo_dir"

if [ -n "$(git status --porcelain --untracked-files=all)" ]; then
  printf 'beta source verification requires a clean worktree\n' >&2
  git status --short >&2
  exit 1
fi

command -v cargo-audit >/dev/null 2>&1 || {
  printf 'cargo-audit is required: cargo install --locked cargo-audit\n' >&2
  exit 1
}
command -v node >/dev/null 2>&1 || {
  printf 'Node.js is required for the frontend beta verification suite\n' >&2
  exit 1
}

cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
node --test web/src/*.test.mjs
cargo audit --deny unsound

printf 'Verified beta source commit %s\n' "$(git rev-parse HEAD)"
