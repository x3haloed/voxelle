# Voxelle IPv6 Field Test

Status: draft  
Audience: operators running the first non-loopback P2P tests  
Goal: learn whether real peers can bootstrap, diagnose, sync, and recover without centralized infrastructure

## What This Test Proves

This test is not a product demo. It is a network truth test.

It should answer:

- Does each machine advertise a usable IPv6 address?
- Does another peer reach the advertised address?
- Does the firewall failure mode show up clearly?
- Can a signed space invite onboard a fresh installation, after which ordinary
  peer records drive diagnosis and sync from the GUI?
- Does a third peer converge after syncing through either earlier peer?
- Can a fresh peer join through an ordinary retaining peer while the original
  inviter is offline?
- Does the Field Test panel make the next action obvious enough?

Loopback success is already covered by tests. The valuable signal here is what
happens off loopback.

## Before You Start

For a source field build on every machine:

```sh
git checkout main
git pull
cargo build -p voxelle-tauri-host
```

For a release field test, install the platform artifact only after verifying
its `VOXELLE-RELEASE.json` as described in `docs/INSTALL_UNSIGNED.md`. Do not
mix homes created by different test roles.

Use isolated homes unless you intentionally want to use the default local home.

```sh
VOXELLE_HOME_ROOT=/tmp/voxelle-peer-a target/debug/voxelle-tauri-host
VOXELLE_HOME_ROOT=/tmp/voxelle-peer-b target/debug/voxelle-tauri-host
VOXELLE_HOME_ROOT=/tmp/voxelle-peer-c target/debug/voxelle-tauri-host
```

On separate machines, the suffix can still be `peer-a`, `peer-b`, or `peer-c`.
The important thing is that each running host has its own home root.

## Surfaces To Watch

Use the ordinary human surfaces first:

- **People** in the header shows the local profile and members, creates and
  copies a signed membership invite with an explicit expiry, lists active
  governance invitations, confirms revocation, and progressively discloses
  identity and manual peer details.
- **Online / Offline** in the header opens Connection & Sync health. It shows
  automatic service, reachability, and synchronization state without requiring
  topology on the ordinary success path. For manual checks, choose the exact
  named peer and verify its IPv6 address, principal, and device before running
  diagnosis or sync; the selection grants no membership or authority.
- **Channels**, **Conversation**, and **Message Composer** select rooms, project
  accepted messages, and send test messages.

For operator intervention, choose **More → Edit layout** and restore the
registered `Runtime Status`, `Network Health`, `Connections`, `Field Test`, and
`Service Activity` views. These expose listen/advertised addresses, imported
ordinary peer records, explicit diagnosis and sync, the re-entrant field-test
checklist, and service errors. The advanced views invoke the same Rust semantic
commands as the focused surfaces; they are not a separate authority path.

Do not conflate the two signed JSON objects. A `.voxinvite` grants membership;
a peer record only advertises replaceable endpoint availability and grants no
membership or protocol authority.

## Two-Peer Base Test

### Peer A

1. Launch the host with an isolated home.
2. Run `Create My Space` if needed.
3. Run `Go Online`.
4. In `Runtime Status`, confirm:
   - `Runtime` is `online`.
   - `Advertise` is not a loopback address unless this is a same-machine test.
5. In `Invite Exchange`, choose an expiry, create a signed space invite, and
   copy its complete JSON. Confirm the invite appears under **Active
   invitations**.
6. Send A's signed space invite to Peer B out-of-band.

As a separate revocation check, create another invite, choose **Revoke
invite…**, review the stale-partition limitation, and confirm. Verify it leaves
**Active invitations** and stays absent after restarting A. While an ordinary
bootstrap peer that has learned the revocation is reachable, verify a fresh
home refuses that stale `.voxinvite` without creating a local identity. Do not
interpret acceptance by an isolated stale partition as strict-single-use or
instantaneous-revocation behavior; neither is claimed.

### Peer B

1. Launch the host with a fresh isolated home; do not initialize a separate
   space first.
2. Choose A's `.voxinvite` file under **Join with an invite**, review the
   displayed space, authority, expiry, and included peers, then run **Join
   Space**. If the invite arrived as complete signed text instead, expand
   **Paste invite JSON instead** and paste it there.
3. Confirm the join creates B's durable principal, admits it to A's space,
   synchronizes retained history, and goes online without manual topology
   steps on the ordinary success path.
4. In `Connection & sync`, select A by its displayed address, principal, and
   device; alternatively, open A's exact row in `Peer List`. Diagnose A and run
   an explicit sync as a re-entrant check. Record the peer-named activity
   result.
5. In `Message Composer`, send a message like:

```text
hello from peer b
```

6. Copy B's ordinary peer record and send it to Peer A if A did not learn B's
   current endpoint during onboarding.

### Peer A Again

1. Import B's peer record if needed; this must not change membership.
   The generic **Import Peer** action must open the Connection & sync review,
   focus the availability input, show B's claimed label/address/principal/device
   and space, and keep Import disabled for incomplete JSON. Compare those claims
   with B's recorded values before importing; Rust remains the validator.
2. In `Connection & sync`, select B and confirm the displayed address,
   principal, and device match B's recorded values. Run the B-named diagnosis
   and sync actions; do not infer the target from peer ordering.
3. Confirm B's message appears in `Room Timeline`.
4. Send a reply from A.
5. Have B sync A again and confirm A's reply appears.
6. Choose a harmless non-empty file no larger than 256 KiB. Before sharing,
   confirm the review names its filename, type, size, `#general`, admitted-space
   audience, and retained-copy limitation. Share it and have B synchronize.
7. On B, confirm the projected filename and size, download it, and compare its
   SHA-256 with A's original. On A, choose **Delete…**, review the tombstone
   limitation, and confirm; after another sync B must project the tombstone.
   Record that this does not erase B's already downloaded copy or the accepted
   signed fact.

## Third-Peer Test

Peer C should prove that the system is not only pairwise happy-path glue.

1. While A is online, create a signed membership invite for C and ensure an
   ordinary retaining peer B has synchronized the current governance/history.
2. Make B's reachable endpoint available as an invite bootstrap hint or other
   ordinary peer hint; the hint must not grant membership.
3. Take A, the invite signer, offline.
4. Launch C with a fresh isolated home and join with A's still-valid signed
   invite through B.
5. Confirm C sees current room messages even though A is offline.
6. Send:

```text
hello from peer c
```

7. Have B import C's ordinary peer record if needed, explicitly select C,
   confirm the import review and C's address/principal/device tuple, diagnose C,
   and sync C.
8. Bring A back online, explicitly select either B or C, and sync that named
   peer. Record which topology edge was exercised.
9. Confirm all three peers can eventually see A, B, and C messages.

## Private-Channel Test

Use the same three admitted peers to exercise confidentiality without creating
a second authority path.

1. On A, create a private channel containing A and B but not C.
2. Record the projected private-member count and key epoch, then send a unique
   harmless marker in that channel.
3. Synchronize B and C. Confirm B can open and read the private channel while C
   does not see it or its marker.
4. On A, choose **Rotate key…**. With the confirmation open, verify it names
   the current private-member count, future-content protection, and the fact
   that earlier retained material cannot be erased. Confirm the rotation.
5. Confirm the projected epoch advances, send a second unique marker, and
   synchronize B and C again. B must read both epochs; C must remain excluded.
6. Restart A and confirm the advanced epoch and private-member count reconstruct
   before sending more private content.

Do not treat rotation as proof that a prior recipient forgot old keys or
plaintext. The test proves current membership-bound distribution and future
epoch use, not remote erasure or forward secrecy.

## What To Record

For a beta-gate run, begin with the release-bound evidence template and follow
`docs/BETA_EVIDENCE.md`. The completed receipt is required in addition to these
human-readable notes; it rejects loopback endpoints, duplicate machines, a
non-offline inviter, missing bidirectional checks, and incomplete message
convergence.

For each peer, write down:

- Machine name or role: A, B, C.
- Home root and IDs from `Profile Summary`.
- Listen and advertised addresses from `Runtime Status`.
- Whether advertised address is loopback, local/private, temporary, or public.
- Which signed membership invite was used and which ordinary peer supplied it
  while the inviter was online or offline.
- Which peer availability record was imported separately.
- Diagnostic result from `Service Activity`.
- Sync result from `Service Activity`.
- Whether new messages appeared after sync.
- Any exact error text.

If possible, capture screenshots of:

- `Profile Summary` and `Runtime Status`
- `Field Test`
- failed `Network Health` rows
- failed `Service Activity` entries

## Interpreting Failures

### Advertised Address Is Loopback

If `Advertise` shows `[::1]` or another loopback-only address, off-machine peers
cannot connect. Try entering a non-loopback IPv6 address in the `Advertise`
field before running `Go Online`.

### Diagnose Fails

This usually means one of:

- the advertised address is not reachable from the other machine,
- a host firewall is blocking inbound UDP,
- the network blocks inbound IPv6,
- the peer went offline,
- the invite contains stale endpoint material.

Record the exact `Service Activity` entry and the advertised address.

### Sync Fails After Diagnose Works

This suggests the transport path exists but the sync request failed. Record the
log text and whether either peer has mismatched room or identity state.

### Sync Works One Way But Not The Other

This is important. It may mean only one machine has inbound reachability. Record
which direction worked:

```text
A -> B diagnose/sync:
B -> A diagnose/sync:
C -> A or B diagnose/sync:
```

### Third Peer Cannot See All Messages

Try syncing C with both A and B. If it still does not converge, record which
peer has which messages. This tells us whether the gap is transport,
anti-entropy, or operator flow.

## Stop Conditions

Stop and patch before continuing if:

- the app panics or exits,
- the UI cannot distinguish or reliably copy/paste membership invites and peer
  availability records,
- the Field Test panel gives a misleading next action,
- failures appear only in terminal output and not in the workbench,
- a peer can diagnose but never sync and the log gives no actionable reason.

## Success Criteria

Minimum useful success:

- A and B can diagnose and sync over non-loopback IPv6.
- C can use A's signed invite to join and receive history through ordinary peer
  B while A is offline.
- A, B, and C can eventually see a message from each peer.
- A and B can exchange content across a private-channel key rotation while C
  remains excluded and the epoch reconstructs after restart.
- The operator can explain what happened using only the workbench panels.

Strong success:

- At least one peer is reachable from another network.
- Firewall or address failures are legible without reading terminal logs.
- The third peer can join by syncing through any existing reachable peer.
