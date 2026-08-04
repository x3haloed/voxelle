# Installing Voxelle Without Vendor Signing Accounts

Voxelle release artifacts are ordinary native Tauri applications. They do not
contain a bundled browser runtime and they do not require an Apple App Store or
Microsoft Store account. Official project artifacts should be accompanied by a
`SHA256SUMS.txt` file generated in the same release job.

## macOS

1. Download the architecture-appropriate or universal `.dmg` and its
   `SHA256SUMS.txt` from the same release.
2. In Terminal, run `shasum -a 256 -c SHA256SUMS.txt` in the download folder.
3. Open the disk image and drag Voxelle to Applications.
4. On first launch, Control-click Voxelle, choose **Open**, then confirm
   **Open**. This records a local exception for the verified copy without
   disabling Gatekeeper globally.

Do not use `sudo spctl --master-disable`. If macOS still quarantines a copy
whose checksum you verified, the narrow fallback is
`xattr -dr com.apple.quarantine /Applications/Voxelle.app`.

## Windows

1. Download the NSIS `.exe` and `SHA256SUMS.txt` from the same release.
2. In PowerShell, compare
   `Get-FileHash .\Voxelle_*.exe -Algorithm SHA256` with the manifest.
3. Run the installer. If SmartScreen appears, choose **More info**, verify that
   the displayed filename matches the checked file, then choose **Run anyway**.

Unsigned code cannot provide a warning-free first launch on all managed Macs or
Windows PCs. Voxelle therefore treats release-page transport plus the published
SHA-256 manifest as the integrity check and documents the narrow per-app trust
exception instead of asking users to weaken system-wide security.

## Building locally

- macOS: `./scripts/package-macos.sh`
- Windows PowerShell: `.\scripts\package-windows.ps1`

The macOS script emits a universal build when both Apple targets are installed;
otherwise it emits a native build. The scripts never request or synthesize a
vendor signing identity.
