# Linux VM Validation

Status: passed compatibility evidence; not a product-envelope expansion.

On 2026-08-22, commit `f5ef3b9` was copied as a clean tracked-source archive
into a fresh disposable vm-stack guest and exercised against the native Linux
desktop dependency graph.

## Environment

- Ubuntu 24.04 server, AArch64
- QEMU 11.1.0 with macOS HVF acceleration
- 4 vCPUs, 6 GiB RAM, 40 GiB copy-on-write disk
- Rust 1.98.0 for build, test, and Clippy; Rustfmt 1.96.0 for the project
  baseline check
- Node.js 18.19.1 with explicit ESM default semantics
- Native WebKitGTK 4.1, GTK 3, SQLite, OpenSSL, and application-indicator
  development dependencies

## Results

- `cargo test --workspace`: passed, 100 tests and all doctests.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `node --experimental-default-type=module --test web/src/*.test.mjs`:
  passed, 20 tests.
- `cargo build --release --workspace`: passed.
- `cargo +1.96.0 fmt --all -- --check`: initially exposed five formatting
  diffs; the repository was formatted, then the check passed.
- Post-format `voxelle-app` and `voxelle-release` tests and strict Clippy:
  passed.
- `file` identified `voxelle`, `voxelle-inhabitantd`,
  `voxelle-tauri-host`, and `voxelle-release` as AArch64 ELF executables.

The host-retained transcript is
`~/.config/vm-stack/evidence/voxelle-linux-validation-2026-08-22.log`
(88,914 bytes, 2,471 lines), with SHA-256
`59af05035fa8d241b804ce3b1059c07b4f0f2a0ab971410fc70b376b102b8f21`.

## Claim boundary

This closes the Linux build, test, lint, and native dependency-resolution
question for this commit and architecture. It does not claim a supported Linux
installer, a lived Linux GUI launch, x86-64 Linux behavior, non-loopback field
reachability, physical media behavior, or any Windows result.

The remaining Windows beta gate requires native x86-64 Windows and a visible
first launch of the signed NSIS artifact. The Apple Silicon host can accelerate
Windows ARM64 only, while `record-windows-beta-smoke.ps1` rejects non-x64
hosts. An ARM VM therefore cannot honestly satisfy that gate.
