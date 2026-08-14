# Beta Evidence Procedure

Status: required external-evidence procedure for the full beta gate.

The signed release manifest proves artifact identity and update authority. It
does not prove that Windows displayed the application, that three physical
machines communicated off loopback, or that signing secrets were moved into
offline custody. Those facts are recorded in one bounded
`voxelle-beta-evidence/v1` document and checked against the authenticated
manifest and reviewed trust roots.

The evidence file is a review record, not protocol authority. Setting a boolean
does not make a test true. Operators must retain the associated screenshots,
machine notes, and failure logs outside release assets; the verifier only makes
missing, inconsistent, or weaker claims fail closed.

## 1. Create the release-bound template

From the reviewed source checkout, use the release manifest downloaded through
the clean readback procedure and the exact commit named by its Git tag:

```sh
cargo run -q -p voxelle-release -- beta-evidence-template \
  --trust-roots release/trusted-update-keys.json \
  --manifest DOWNLOAD_DIR/VOXELLE-RELEASE.json \
  --source-commit FULL_40_CHARACTER_TAG_COMMIT \
  --output beta-evidence.template.json
```

The command authenticates the manifest before copying its release ID, sequence,
Windows installer name and digest, manifest-signing key ID, and recovery-key ID
into the template. It refuses to overwrite an existing file.

Fill the `distribution` section from the clean public readback and packaged
macOS run. It requires the exact GitHub tag URL, authenticated public readback,
DMG verification, universal Mach-O inspection, packaged launch, live activation,
rollback to the previous signed generation, and reactivation of the current
generation. These are separate facts even when performed by one operator.

## 2. Record native Windows first launch

On an x86-64 Windows machine, verify and install the exact NSIS asset listed by
the signed manifest. Then run from the same reviewed checkout:

```powershell
.\scripts\record-windows-beta-smoke.ps1 `
  -Template .\beta-evidence.template.json `
  -Output .\beta-evidence.windows.json `
  -Installer .\Voxelle_0.1.0_x64-setup.exe `
  -InstalledExecutable "$env:LOCALAPPDATA\Voxelle\voxelle-tauri-host.exe" `
  -Operator "OPERATOR NAME"
```

Use the actual installed executable path if NSIS selected a different location.
The script refuses non-Windows and non-x64 hosts, checks the installer SHA-256
against the signed template, launches the installed executable, and requires a
live process with a visible titled main window. It leaves Voxelle open for
human inspection and writes a new evidence file without changing the template.

## 3. Record the three-machine field test

Perform every step in `docs/FIELD_TEST.md` using three distinct machines and
homes. In the `field` object of the Windows receipt, record:

- roles A, B, and C with distinct opaque machine fingerprints, principal IDs,
  and device IDs;
- actual bracketed IPv6 socket addresses, including ports, for listen and
  advertised endpoints;
- successful diagnosis and sync in both A-to-B and B-to-A directions;
- A offline while C joins through retaining peer B and sees retained history;
- one unique harmless message marker authored by each role, with all three
  roles listed as observers.

The verifier rejects IPv4, loopback, multicast, and documentation-only
`2001:db8::/32` addresses. A listen address may be the normal IPv6 wildcard
`[::]:PORT`, but an advertised address must identify an actual usable interface
and cannot be unspecified.

## 4. Establish signing-secret custody

The ordinary release key and recovery-only key carry different capabilities and
must be stored on separately protected offline media. For each restored copy,
while its medium is mounted, verify the secret against the reviewed trust roots:

```sh
cargo run -q -p voxelle-release -- verify-signing-secret \
  --trust-roots release/trusted-update-keys.json \
  --secret /MOUNT/RELEASE/signing-key.json \
  --role release

cargo run -q -p voxelle-release -- verify-signing-secret \
  --trust-roots release/trusted-update-keys.json \
  --secret /OTHER_MOUNT/RECOVERY/recovery-signing-key.json \
  --role recovery
```

The verifier rejects symlinks, non-regular files, capability-role mismatches,
public-key mismatches, and, on Unix, group/other-accessible permissions. After a
successful restore test, unmount both media and remove the development-host
copies only when the operator has confirmed the offline copies are recoverable.
That deletion is intentionally manual and destructive.

Record non-secret storage descriptions, separate protection, offline state,
development-copy removal, restore-test completion, timestamp, and operator in
the `custody` object. Never put paths containing credentials, passwords,
recovery material, or secret bytes in the evidence document.

## 5. Verify the complete gate

```sh
cargo run -q -p voxelle-release -- verify-beta-evidence \
  --trust-roots release/trusted-update-keys.json \
  --manifest DOWNLOAD_DIR/VOXELLE-RELEASE.json \
  --evidence beta-evidence.complete.json \
  --expected-commit FULL_40_CHARACTER_TAG_COMMIT
```

Success means the receipt is complete and internally consistent with the
authenticated release. It does not convert operator attestations into
cryptographic proof or make GitHub an authority. Commit the completed receipt
only after reviewing it for secrets and retaining the supporting evidence.
