# Installing Voxelle Without Vendor Signing Accounts

Voxelle release artifacts are ordinary native Tauri applications. They do not
contain a bundled browser runtime and they do not require an Apple App Store or
Microsoft Store account. Official project artifacts are published through
GitHub Releases and must be listed in a signed `VOXELLE-RELEASE.json` manifest.

`SHA256SUMS.txt` remains a convenient corruption check, but it is not
authenticity evidence when served beside an artifact. The installed kernel and
the `voxelle-release` verifier authenticate release manifests and product
updates against the repository's Ed25519 release trust root.

## Verify a signed release first

From a reviewed Voxelle source checkout, place all downloaded assets in one
directory and run:

```sh
cargo run -q -p voxelle-release -- verify-release \
  --trust-roots release/trusted-update-keys.json \
  --manifest DOWNLOAD_DIR/VOXELLE-RELEASE.json \
  --artifact-dir DOWNLOAD_DIR
```

The verifier checks the manifest signature plus every artifact name, byte
length, and SHA-256 digest. It does not trust GitHub, a mirror, or the
manifest's own claimed signer unless that signer is present in the reviewed
trust-root file.

The desktop Product Update view applies the same root verification to a signed
`.voxupdate` package before live activation.

## macOS

1. Download the universal `.dmg`, `VOXELLE-RELEASE.json`, and optional
   `SHA256SUMS.txt` from the same release.
2. Verify the signed release as above. You may additionally run
   `shasum -a 256 -c SHA256SUMS.txt` as a transport-corruption check.
3. Open the disk image and drag Voxelle to Applications.
4. On first launch, Control-click Voxelle, choose **Open**, then confirm
   **Open**. This records a local exception for the verified copy without
   disabling Gatekeeper globally.

Do not use `sudo spctl --master-disable`. If macOS still quarantines a copy
whose signed manifest you verified, the narrow fallback is
`xattr -dr com.apple.quarantine /Applications/Voxelle.app`.

## Windows

1. Download the NSIS `.exe`, `VOXELLE-RELEASE.json`, and optional
   `SHA256SUMS.txt`.
2. Verify the signed release. You may additionally compare
   `Get-FileHash .\Voxelle_*.exe -Algorithm SHA256` with `SHA256SUMS.txt`.
3. Run the installer. If SmartScreen appears, choose **More info**, verify that
   the displayed filename matches the checked file, then choose **Run anyway**.

Unsigned code cannot provide a warning-free first launch on all managed Macs or
Windows PCs. Voxelle authenticates release contents independently of the
release page and documents narrow per-app exceptions rather than asking users
to weaken system-wide security.

## Building and publishing manually

- macOS: `./scripts/package-macos.sh`
- Windows PowerShell: `.\scripts\package-windows.ps1`
- Product generation:
  `scripts/prepare-product-update.sh RELEASE_ID SEQUENCE OUTPUT_DIR`
- Signed release manifest:
  `scripts/sign-release.sh RELEASE_ID SEQUENCE VOXELLE-RELEASE.json ARTIFACT...`
- Signed release-root transition:
  `cargo run -p voxelle-release -- sign-trust-transition --secret OLD_KEY --output ROTATION.voxtrust --sequence N --add-trust-roots NEW_PUBLIC_ROOTS --remove-key-id OLD_KEY_ID`
- Trust-transition verification:
  `cargo run -p voxelle-release -- verify-trust-transition --trust-roots EMBEDDED_ROOTS --transition ROTATION.voxtrust --state-dir TRUST_STATE`
- GitHub Release publication:
  `scripts/publish-github-release.sh TAG TITLE ASSET...`

The macOS script emits a universal build when both Apple targets are installed;
otherwise it emits a native build. The scripts never request or synthesize a
vendor signing identity. There is intentionally no formal release CI/CD for the
beta path: a release operator runs the tests, builds both platform artifacts,
signs one manifest over the collected bytes, verifies it from a clean
directory, publishes with `gh`, and reads the published assets back.

The release signing secret defaults to
`~/.config/voxelle-release/signing-key.json`, must remain mode `0600` on Unix,
must never enter the repository or release assets, and needs a separately
protected offline backup. Rotate proactively by generating the successor,
signing a `.voxtrust` transition with the current key, independently verifying
it, applying it on a test installation, and only then retiring the old secret.
Losing the only currently trusted secret before signing a successor transition
prevents future releases from authenticating to installed beta kernels.
Disclosing it permits malicious updates until a legitimate already-trusted key
signs a transition that retires it.

The separately generated recovery secret at
`~/.config/voxelle-release/recovery-signing-key.json` is not an ordinary release
key: the kernel rejects it for manifests and product packages and accepts it
only for emergency trust transitions. Move both signing secrets to separately
protected offline storage before publishing a beta; their default paths are
development locations, not an offline-backup claim.

Inside the app, **Check GitHub Releases** downloads only the bounded latest
manifest. Voxelle shows an update as available only after authenticating that
manifest. **Download and Stage Update** then verifies the exact signed artifact
without changing the running product. **Activate Staged Update** is a separate
explicit action; **Discard Staged Update** removes it without changing the
active generation. Network, HTTP, redirect, checksum, signer, compatibility,
and semantic-validation failures remain visible in the Product Update view.
