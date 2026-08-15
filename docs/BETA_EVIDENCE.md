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

After completing them, authenticate the same downloaded manifest and record
the distribution section into a new staged receipt:

```sh
cargo run -q -p voxelle-release -- record-distribution-beta-evidence \
  --input CURRENT_RECEIPT.json \
  --output beta-evidence.distribution.json \
  --trust-roots release/trusted-update-keys.json \
  --manifest DOWNLOAD_DIR/VOXELLE-RELEASE.json \
  --executed-utc 2026-08-14T22:00:00Z \
  --operator "OPERATOR NAME" \
  --attest-public-readback-verified \
  --attest-macos-dmg-verified \
  --attest-macos-universal-binary \
  --attest-macos-packaged-launch \
  --attest-live-activation \
  --attest-rollback-to-previous \
  --attest-reactivated-current
```

The recorder authenticates the manifest, derives its exact release tag URL,
requires every packaged behavior separately, checks that the staged receipt
identifies the same release and sequence, preserves other sections, and refuses
output overwrite. It records the operator's observations; it does not perform
the public readback, launch, activation, or rollback itself.

During the packaged update lifecycle, inspect Product Updates and the command
palette before and after each transition. Confirm download and stage is
unavailable until a signed release is discovered; activation and discard are
unavailable until that generation is staged; and rollback is unavailable until
a previous verified generation exists. Each unavailable action must name its
missing prerequisite, and each action must become available from the
Rust-projected state after its prerequisite completes—not from a frontend-only
assumption.

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

Before importing the first peer record on one role, confirm Diagnose Peer and
Sync Peer are unavailable in the command palette and Network Health with **Join
with an invite or import peer availability first**, while Import Peer remains
available and opens the bounded availability review. Do not count this local
unavailable-state check as field connectivity evidence.

When a peer availability record must be imported, begin from the generic
**Import Peer** action. Confirm it opens and focuses the bounded review rather
than submitting an empty hidden draft. Record the claimed label, IPv6 address,
principal, device, and space; compare them with the source role; and confirm
incomplete JSON cannot be submitted. These are untrusted presentation claims:
only the native kernel stores a validated record, and synchronization still
refuses a record whose authority does not match the active home.

After the three-machine observations are complete, record the field section
from the latest partial receipt. Substitute the exact values copied from each
role; quote bracketed IPv6 sockets in shells that interpret brackets:

```sh
cargo run -q -p voxelle-release -- record-field-beta-evidence \
  --input CURRENT_RECEIPT.json \
  --output beta-evidence.field.json \
  --executed-utc 2026-08-14T21:00:00Z \
  --operator "OPERATOR NAME" \
  --machine-a-fingerprint A_FINGERPRINT \
  --machine-a-principal A_PRINCIPAL_ID \
  --machine-a-device A_DEVICE_ID \
  --machine-a-listen '[::]:47000' \
  --machine-a-advertise '[REAL_A_IPV6]:47000' \
  --machine-b-fingerprint B_FINGERPRINT \
  --machine-b-principal B_PRINCIPAL_ID \
  --machine-b-device B_DEVICE_ID \
  --machine-b-listen '[::]:47001' \
  --machine-b-advertise '[REAL_B_IPV6]:47001' \
  --machine-c-fingerprint C_FINGERPRINT \
  --machine-c-principal C_PRINCIPAL_ID \
  --machine-c-device C_DEVICE_ID \
  --machine-c-listen '[::]:47002' \
  --machine-c-advertise '[REAL_C_IPV6]:47002' \
  --message-a-marker A_UNIQUE_MARKER \
  --message-b-marker B_UNIQUE_MARKER \
  --message-c-marker C_UNIQUE_MARKER \
  --attest-a-to-b-diagnose \
  --attest-b-to-a-diagnose \
  --attest-a-to-b-sync \
  --attest-b-to-a-sync \
  --attest-inviter-a-offline \
  --attest-c-joined-through-b \
  --attest-c-retained-history-visible \
  --attest-a-message-visible-on-all \
  --attest-b-message-visible-on-all \
  --attest-c-message-visible-on-all
```

The recorder rejects duplicate machine, principal, or device identities;
IPv4, loopback, multicast, unspecified advertised, and documentation-only IPv6
addresses; incomplete bidirectional checks; a non-offline inviter; and missing
three-way message convergence. It validates before replacing the field section,
preserves all other sections, and refuses to overwrite an output. As with the
human recorder, flags are operator attestations rather than automated network
observations.

## 4. Record human causal-path evidence

Use an actual supported desktop and name the assistive technology in the
`human.assistive_technology` object (for example, VoiceOver, Narrator, or NVDA).
With that technology active and without using a pointer, complete and record:

- fresh setup and signed-invite join;
- reading and sending conversation content;
- recovery and lost-device revocation;
- dock placement or visibility customization that survives restart;
- understanding and acting on a degraded connection state;
- navigating the narrowest practical window without hidden keyboard targets or
  document-level horizontal scrolling; and
- entering, operating, and leaving the direct-media surface.

If the initial native snapshot fails during this run, confirm the startup alert
keeps technical details behind an actionable disclosure, focuses **Try Again**,
states that retry does not delete, archive, or replace local state, and reaches
the ordinary product after the underlying issue is corrected. Do not induce a
credential or storage failure solely to satisfy this conditional observation.

In Customize, confirm every unchanged preference begins with its contextual
Save action disabled, changing the associated control enables only that Save
action, and accepting it returns the action to disabled with the projected
value retained. Invoke **Reset All Customization** and confirm a focused modal
review names appearance, spacing, behavior, and dock placement or visibility,
states that protocol state is untouched, traps Tab and Shift+Tab, and returns
to the reset action after Escape or **Keep my customization** without changing
the current customization. Confirm the reviewed reset through the ordinary
native command only when the test intends to restore all defaults.
Invoke **Reset Workbench Layout** from the command palette and confirm its
separate focused review names only dock placement and visibility, explicitly
preserves appearance, spacing, and behavior, and returns to the rendered reset
control or visible **More** entry point after cancellation without changing
either layout or preference drafts.

Before creating an invite, change among the bounded expiry choices and confirm
the live review names the selected signed bearer-capability window and the
button says **Create signed invite**. After creation, confirm the projected
absolute expiry matches that choice.
Create two active invites whose expiries render identically; confirm their row
actions remain distinct and the chosen invite marker persists through the
revocation confirmation and final Revoke button while the other invite remains
unchanged after cancellation.

In **Connection & sync**, enter an advertised IPv6 address without brackets or
a port, then a bracketed address with port `70000`. Invoke **Go Online** each
time and confirm the exact field is announced invalid, receives focus, and the
service does not reconfigure. Correct it to a real bracketed IPv6 socket with a
valid port and continue through the native connection path. Leaving both fields
empty must retain automatic local setup.

While the peer service is online, confirm the command palette disables **Go
Online** with a direction to Connection & sync and keeps **Go Offline**
available. Confirm the explicit **Go Online** action inside Connection & sync
remains available for applying visible Bind or Advertise drafts. After going
offline, confirm the palette exposes Go Online and disables Go Offline as
already offline.

Before sending conversation content, confirm **Send Message** is disabled for
an empty or whitespace-only draft, becomes enabled for visible text or a member
inserted through **Mention someone**, and disables again when the draft is
cleared. Open inline editing and confirm **Save changes** follows the same
empty, whitespace, visible-content, and cleared states. Then send and edit
through the ordinary admitted paths. Confirm two emoji count as 2 rather than 4,
leading or trailing whitespace produces corrective guidance, and a 4,001
character draft stays visible but cannot be sent until shortened.

Open retained search and confirm **Search Messages** follows the same disabled
empty, whitespace, visible-term, and cleared states. Paste a multi-line query
and then 1,025 Unicode characters; confirm the live guidance names the relevant
correction, retains the draft, and does not claim a search ran. In profile
editing, submit a whitespace-only display name; confirm the field is announced
invalid, receives focus with **Enter a display name**, and clears that stale
error when corrected.
Also exercise a leading-space profile name, an 81-character channel name, and a
trailing-space role name; confirm each form names and focuses its own correction
without claiming the command was admitted.
Enter 513 Unicode characters in profile About and 1,025 in channel Topic;
confirm each optional field names its own limit and receives focus, while an
empty optional field remains valid.

Choose an attachment whose filename starts or ends with whitespace or exceeds
255 Unicode characters. Confirm Voxelle does not read or review the bytes, asks
for a rename, and states that nothing was shared. After renaming, confirm the
ordinary focused attachment review and cancel/share paths still work.

After restoring identity on the fresh device, confirm the success status states
that authority from previous devices was revoked and directs the person to save
a fresh offline recovery kit. Keyboard focus must land on that fresh-kit action,
including after dismissing the status. Save the kit, then confirm the Identity
Recovery view shows when it was last saved and offers **Save a fresh recovery
kit** without displaying or retaining its filesystem path.

While operating that surface, mute and unmute the local microphone through the
named call control and through the command palette. Confirm the assistive
technology announces the resulting **Microphone muted** and **Microphone on**
states. If the captured track disappears, confirm the unavailable state directs
the person to leave and rejoin rather than claiming that audio resumed.
Turn an already captured camera off and back on through the named call control
and command palette. Confirm the local and remote participant states change
accordingly and are announced. From a voice-only join, invoke the camera control
and confirm it directs the person to leave and rejoin with camera rather than
claiming that capture started.

Do not enter `none`, `unknown`, or a generic claim. Accessibility-tree tests,
keyboard automation, and deterministic media mocks are valuable development
evidence, but they do not satisfy this actual assistive-technology gate.

During those keyboard-only paths, invoke at least one form-backed action from
the command palette and confirm focus lands in the required visible input. Also
inspect one unavailable command and confirm the assistive technology announces
its missing prerequisite rather than allowing an avoidable failing action. On a
fresh home, invoke the **Focus Message Composer** shortcut and confirm the
palette keeps the command visible but disabled with **Create, join, or recover a
space first** instead of closing without moving focus.
In the channel list, conversation, and People surface, navigate repeated
channel, message, reaction, member, role, and invitation controls. Confirm each
announced name identifies the visible target rather than relying on position
alone. Have two current members use the same display name with different case;
confirm their cards, mention choices, Ban/Unban controls, and role-assignment
controls retain distinct matching member markers through confirmation. Create
two roles with the same name but different permissions; confirm their cards,
Manage members controls, assignment actions, and reviews retain distinct
matching role markers and permission descriptions. Create two channels with the
same name, including one private channel; confirm their cards, Select and key
rotation controls, header, conversation, composer, search results, and
notification actions retain distinct matching channel markers. Select a
different channel and confirm the accepted selected row is
announced as the current location and receives focus after its Select button
disappears. Add and remove a reaction, then pin and unpin a message. Confirm the
replacement control keeps focus, names its message target, and exposes the
opposite admitted action; if the last visible reaction disappears, focus may
fall back to that message row. Close one transient panel with Escape and confirm
focus returns to its invoking header control with the collapsed state announced.
Have one author post identical text twice. Confirm both message rows retain
distinct matching markers across reactions, Message actions, Reply, Edit,
Delete, and deletion review; repeat with identical attachment filenames and
confirm the Download and Delete-file labels remain distinct.
Open at least one disclosure with Enter and another with Space. Confirm the
assistive technology announces each as an actionable control and reports the
collapsed or expanded state after activation. In channel creation, turn private
mode on and then off; confirm **Private channel options** closes when its private
fields no longer apply. Confirm profile, peer-import, channel-create, and
role-create disclosures close after their successful commands, while unrelated
user-expanded disclosures stay open across an ordinary refresh.
Confirm the channel and role disclosure controls announce **Open channel
creation form** and **Open role creation form**, distinct from the nested
**Create Channel** and **Create Role** submit actions.

Resize the supported desktop window to its narrowest practical width during one
conversation path. Confirm the header actions, selected conversation, composer,
and transient Connection or utility action remain keyboard-reachable without
document-level horizontal scrolling. At compact width, confirm header actions
and composer controls form bounded two-column rows rather than pushing one
routine action onto every row. Record the window size and any wrapping or
stacking behavior; this operator observation complements rather than replaces
the assistive-technology evidence above.
If a transient surface becomes modal at that width, confirm it is announced as
modal, Tab and Shift+Tab remain among visible controls (including collapsed
disclosure summaries but not their hidden contents), and Escape returns to the
invoking header action.

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

After completing those observations, record the human section without manually
editing nested JSON. Start from the latest partial receipt so completed Windows,
field, or distribution sections remain intact. Every `--attest-*` flag is
required separately; omission fails before creating the output:

```sh
cargo run -q -p voxelle-release -- record-human-beta-evidence \
  --input CURRENT_RECEIPT.json \
  --output beta-evidence.human.json \
  --executed-utc 2026-08-14T20:00:00Z \
  --operator "OPERATOR NAME" \
  --platform macOS \
  --technology VoiceOver \
  --media-role A B \
  --attest-keyboard-only \
  --attest-fresh-setup \
  --attest-invite-join \
  --attest-conversation \
  --attest-recovery \
  --attest-customization \
  --attest-degraded-connection \
  --attest-compact-window-navigation \
  --attest-media-controls \
  --attest-microphone-toggle-controls \
  --attest-camera-toggle-controls \
  --attest-physical-microphone-capture \
  --attest-physical-camera-capture \
  --attest-permission-denial-recovery \
  --attest-direct-audio-observed-by-all \
  --attest-direct-video-observed-by-all \
  --attest-direct-connection-state-visible \
  --attest-leave-stopped-capture \
  --attest-missing-peer-state-visible
```

Use `--platform Windows --technology Narrator` (or the actual named Windows
technology) when that is the tested surface. Roles must be two or three distinct
members of A, B, and C from the same receipt. The recorder validates the human
section, preserves all other sections, and refuses to overwrite an existing
output. It improves recording accuracy; it does not observe the test or turn an
operator assertion into proof.

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

Only after the manual removal and restore checks are complete, record custody
into a new staged receipt. Storage values are non-secret descriptions, not
filesystem paths:

```sh
cargo run -q -p voxelle-release -- record-custody-beta-evidence \
  --input CURRENT_RECEIPT.json \
  --output beta-evidence.custody.json \
  --trust-roots release/trusted-update-keys.json \
  --manifest DOWNLOAD_DIR/VOXELLE-RELEASE.json \
  --release-storage "NON-SECRET RELEASE MEDIUM DESCRIPTION" \
  --recovery-storage "NON-SECRET RECOVERY MEDIUM DESCRIPTION" \
  --attested-utc 2026-08-14T23:00:00Z \
  --operator "OPERATOR NAME" \
  --attest-separately-protected \
  --attest-offline \
  --attest-development-copies-removed \
  --attest-restore-tested
```

The recorder authenticates the manifest and trust roots, derives the ordinary
release and recovery-only key IDs from their distinct capability roles,
requires distinct bounded storage descriptions and every custody observation,
preserves other sections, and refuses output overwrite. It never reads, moves,
unmounts, or deletes a signing secret. Those operations remain manual because
mistaken automation here could destroy release authority.

## 6. Verify the complete gate

At any point while assembling the staged receipt, inspect every gate in one
authenticated pass:

```sh
cargo run -q -p voxelle-release -- beta-evidence-status \
  --trust-roots release/trusted-update-keys.json \
  --manifest DOWNLOAD_DIR/VOXELLE-RELEASE.json \
  --evidence CURRENT_RECEIPT.json \
  --expected-commit FULL_40_CHARACTER_TAG_COMMIT
```

The status command authenticates the release manifest, prints `PASS` or `FAIL`
for release identity, distribution, Windows, field, human, and custody evidence,
and exits unsuccessfully until every section is complete and internally
consistent. It reports all section failures at once. Like the strict verifier,
it checks the receipt—not whether an operator's lived observations were true.

Once every status line passes, run the strict gate:

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
