# Releases (GitHub publisher)

Voxelle’s “refresh-to-update” uses a **GitHub Releases** feed. The desktop app fetches a manifest from:

`https://github.com/<owner>/<repo>/releases/latest/download/voxelle-web-manifest.json`

The manifest points at a versioned web bundle zip and includes its sha256.

## Required web assets

Each release should include:

- `voxelle-web-manifest.json` (schema `v=1`)
- `voxelle-web-bundle.zip` (zip that extracts `index.html` at the bundle root)

## Desktop assets (Tauri)

Releases can also include the desktop artifacts produced by `tauri build` (DMG, etc).

## CI workflow

This repo includes a starter workflow to build and attach assets:

- `.github/workflows/release.yml`

Run it from GitHub Actions via “workflow_dispatch” and provide a version like `0.1.0`.

Notes:
- The workflow builds **unsigned** desktop artifacts on macOS + Windows.
- For shipping macOS builds to normal users, we’ll need to add Apple signing + notarization secrets (Team ID + credentials) and configure Tauri’s signing settings.
- For Windows, code signing is also recommended for beta; we can defer it until later if needed.
