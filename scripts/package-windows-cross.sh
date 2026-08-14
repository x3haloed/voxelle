#!/bin/sh
set -eu

repo_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
target=x86_64-pc-windows-msvc

command -v cargo-xwin >/dev/null 2>&1 || {
  printf 'cargo-xwin is required: cargo install --locked cargo-xwin\n' >&2
  exit 1
}
command -v makensis >/dev/null 2>&1 || {
  printf 'NSIS is required (Homebrew formula: makensis).\n' >&2
  exit 1
}

llvm_bin=${VOXELLE_LLVM_BIN:-}
if [ -z "$llvm_bin" ] && command -v brew >/dev/null 2>&1; then
  candidate=$(brew --prefix llvm 2>/dev/null || true)
  if [ -x "$candidate/bin/llvm-lib" ]; then
    llvm_bin="$candidate/bin"
  fi
fi
if [ -z "$llvm_bin" ] || [ ! -x "$llvm_bin/llvm-lib" ] || [ ! -x "$llvm_bin/llvm-rc" ]; then
  printf 'set VOXELLE_LLVM_BIN to an LLVM bin directory containing llvm-lib and llvm-rc\n' >&2
  exit 1
fi

rustup target list --installed | grep -qx "$target" || {
  printf 'Rust target %s is required.\n' "$target" >&2
  exit 1
}

cross_path="$llvm_bin:$(dirname -- "$(command -v makensis)"):$PATH"
xwin_cache=${VOXELLE_XWIN_CACHE_DIR:-"$repo_dir/target/xwin-cache"}
llvm_dyld=${VOXELLE_LLVM_DYLD_PATH:-}

cd "$repo_dir/crates/voxelle-tauri-host"
PATH="$cross_path" \
DYLD_LIBRARY_PATH="$llvm_dyld" \
XWIN_CACHE_DIR="$xwin_cache" \
  cargo tauri build --runner cargo-xwin --target "$target"

bundle_dir="$repo_dir/target/$target/release/bundle/nsis"
app_exe="$repo_dir/target/$target/release/voxelle-tauri-host.exe"
pe_headers=$(DYLD_LIBRARY_PATH="$llvm_dyld" "$llvm_bin/llvm-readobj" --file-headers "$app_exe")
printf '%s\n' "$pe_headers" | grep -q 'Format: COFF-x86-64'
printf '%s\n' "$pe_headers" | grep -q 'Subsystem: IMAGE_SUBSYSTEM_WINDOWS_GUI'
(cd "$bundle_dir" && shasum -a 256 ./*.exe > SHA256SUMS.txt)
printf 'Built cross-compiled Windows NSIS release in %s\n' "$bundle_dir"
printf 'Native Windows install and first-launch verification are still required.\n'
