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
  topology on the ordinary success path.
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
4. In `Peer List`, diagnose A and run an explicit sync as a re-entrant check.
5. In `Message Composer`, send a message like:

```text
hello from peer b
```

6. Copy B's ordinary peer record and send it to Peer A if A did not learn B's
   current endpoint during onboarding.

### Peer A Again

1. Import B's peer record if needed; this must not change membership.
2. Run `Diagnose Peer` and `Sync Peer`.
3. Confirm B's message appears in `Room Timeline`.
4. Send a reply from A.
5. Have B sync A again and confirm A's reply appears.

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

7. Have B import C's ordinary peer record if needed, diagnose C, and sync C.
8. Bring A back online and sync it against either B or C.
9. Confirm all three peers can eventually see A, B, and C messages.

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
- The operator can explain what happened using only the workbench panels.

Strong success:

- At least one peer is reachable from another network.
- Firewall or address failures are legible without reading terminal logs.
- The third peer can join by syncing through any existing reachable peer.
