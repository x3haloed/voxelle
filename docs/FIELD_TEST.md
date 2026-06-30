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
- Can invite JSON be copied, imported, diagnosed, and synced from the GUI?
- Does a third peer converge after syncing through either earlier peer?
- Does the Field Test panel make the next action obvious enough?

Loopback success is already covered by tests. The valuable signal here is what
happens off loopback.

## Before You Start

On every machine:

```sh
git checkout ipv6
git pull
cargo build -p voxelle-tauri-host
```

Use isolated homes unless you intentionally want to use the default local home.

```sh
VOXELLE_HOME_ROOT=/tmp/voxelle-peer-a target/debug/voxelle-tauri-host
VOXELLE_HOME_ROOT=/tmp/voxelle-peer-b target/debug/voxelle-tauri-host
VOXELLE_HOME_ROOT=/tmp/voxelle-peer-c target/debug/voxelle-tauri-host
```

On separate machines, the suffix can still be `peer-a`, `peer-b`, or `peer-c`.
The important thing is that each running host has its own home root.

## Surfaces To Watch

Use these panels in the workbench:

- `Home`: confirms the current home root, peer ID, device ID, runtime state,
  listen address, advertised address, known peer count, and message count.
- `Network Health`: shows lower-level setup and reachability rows.
- `Peer Exchange`: copy local invite JSON and import another peer's invite JSON.
- `Field Test`: shows the operator checklist for the current peer.
- `Network Log`: shows diagnostic, sync, service, and error events.
- `Room`: send and read test messages.

The thing to copy is the full JSON in `Peer Exchange` after the peer is online.

## Two-Peer Base Test

### Peer A

1. Launch the host with an isolated home.
2. In `Field Test`, run `Initialize Home` if needed.
3. Run `Go Online`.
4. In `Home`, confirm:
   - `Runtime` is `online`.
   - `Advertise` is not a loopback address unless this is a same-machine test.
5. In `Peer Exchange`, copy the local invite JSON.
6. Send A's invite JSON to Peer B out-of-band.

### Peer B

1. Launch the host with an isolated home.
2. Run `Initialize Home`.
3. Paste A's invite JSON into `Peer Exchange`.
4. Run `Import`.
5. In `Field Test`, run `Diagnose Peer`.
6. If diagnose works, run `Sync Peer`.
7. In `Room`, send a message like:

```text
hello from peer b
```

8. Run `Go Online`.
9. Copy B's invite JSON and send it to Peer A.

### Peer A Again

1. Import B's invite JSON.
2. Run `Diagnose Peer`.
3. Run `Sync Peer`.
4. Confirm B's message appears in `Room`.
5. Send a reply from A.
6. Have B sync A again and confirm A's reply appears.

## Third-Peer Test

Peer C should prove that the system is not only pairwise happy-path glue.

1. Launch Peer C with its own home.
2. Initialize Peer C.
3. Import either A's invite or B's invite.
4. Diagnose the imported peer.
5. Sync the imported peer.
6. Confirm C sees the current room messages.
7. Send:

```text
hello from peer c
```

8. Have A or B import C's invite.
9. Diagnose C and sync C.
10. Have the remaining peer sync against either peer that has C's message.
11. Confirm all three peers can eventually see A, B, and C messages.

## What To Record

For each peer, write down:

- Machine name or role: A, B, C.
- Home root from `Home`.
- Advertised address from `Home`.
- Whether advertised address is loopback, local/private, temporary, or public.
- Which peer invite was imported.
- Diagnostic result from `Network Log`.
- Sync result from `Network Log`.
- Whether new messages appeared after sync.
- Any exact error text.

If possible, capture screenshots of:

- `Home`
- `Field Test`
- failed `Network Health` rows
- failed `Network Log` entries

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

Record the exact `Network Log` entry and the advertised address.

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
- the UI cannot copy/import invite JSON reliably,
- the Field Test panel gives a misleading next action,
- failures appear only in terminal output and not in the workbench,
- a peer can diagnose but never sync and the log gives no actionable reason.

## Success Criteria

Minimum useful success:

- A and B can diagnose and sync over non-loopback IPv6.
- C can import either A or B and receive room history.
- A, B, and C can eventually see a message from each peer.
- The operator can explain what happened using only the workbench panels.

Strong success:

- At least one peer is reachable from another network.
- Firewall or address failures are legible without reading terminal logs.
- The third peer can join by syncing through any existing reachable peer.
