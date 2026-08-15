# Beta Evidence Procedure

Status: required external-evidence procedure for the full beta gate.

The signed release manifest proves artifact identity and update authority. It
does not prove that Windows displayed the application, that three physical
machines communicated off loopback, that the human paths worked with actual
assistive technology and physical media devices, or that signing secrets were
moved into offline custody. Those facts are recorded in one bounded
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

For every manual diagnosis or sync, use **Connection & sync** to select the
named target and compare its displayed address, principal, and device with the
role record before invoking the peer-named action. Retain the corresponding
peer-named Service Activity result. A generic success against whichever peer
happened to be stored first is not directional field evidence.

When a peer availability record must be imported, begin from the generic
**Import Peer** action. Confirm it opens and focuses the bounded review rather
than submitting an empty hidden draft. Record the claimed label, IPv6 address,
principal, device, and space; compare them with the source role; and confirm
incomplete JSON cannot be submitted. These are untrusted presentation claims:
only the native kernel stores a validated record, and synchronization still
refuses a record whose authority does not match the active home.

## 4. Record human causal-path evidence

Use an actual supported desktop and name the assistive technology in the
`human.assistive_technology` object (for example, VoiceOver, Narrator, or NVDA).
With that technology active and without using a pointer, complete and record:

- fresh setup and signed-invite join;
- reading and sending conversation content;
- recovery and lost-device revocation;
- dock placement or visibility customization that survives restart;
- understanding and acting on a degraded connection state; and
- entering, operating, and leaving the direct-media surface.

Do not enter `none`, `unknown`, or a generic claim. Accessibility-tree tests,
keyboard automation, and deterministic media mocks are valuable development
evidence, but they do not satisfy this actual assistive-technology gate.

During those keyboard-only paths, invoke at least one form-backed action from
the command palette and confirm focus lands in the required visible input. Also
inspect one unavailable command and confirm the assistive technology announces
its missing prerequisite rather than allowing an avoidable failing action.
In the channel list, conversation, and People surface, navigate repeated
channel, message, reaction, member, role, and invitation controls. Confirm each
announced name identifies the visible target rather than relying on position
alone. Close one transient panel with Escape and confirm focus returns to its
invoking header control with the collapsed state announced.
Open at least one disclosure with Enter and another with Space. Confirm the
assistive technology announces each as an actionable control and reports the
collapsed or expanded state after activation.

Resize the supported desktop window to its narrowest practical width during one
conversation path. Confirm the header actions, selected conversation, composer,
and transient Connection or utility action remain keyboard-reachable without
document-level horizontal scrolling. Record the window size and any wrapping or
stacking behavior; this operator observation complements rather than replaces
the assistive-technology evidence above.

Then use two or three distinct physical participants drawn from the three
machine roles in the field receipt. In `human.media`, record the participating roles
and confirm all of the following with real microphones and cameras:

- microphone and camera capture both work;
- denying a device permission produces an explicit state and the user can
  recover after granting permission;
- every participant observes direct audio and video from the intended peers;
- each participant can perceive the direct connection state;
- leaving stops local capture; and
- a missing or crashed peer remains an explicit state rather than appearing as
  a healthy participant.

Retain screenshots, assistive-technology notes, device/OS details, and failure
observations with the other supporting evidence. The receipt contains only the
bounded operator attestation and must not contain recordings or private
conversation content.

## 5. Establish signing-secret custody

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

## 6. Verify the complete gate

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
