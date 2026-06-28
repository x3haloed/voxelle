# IPv6 Branch Roadmap

Status: active implementation roadmap  
Branch: `ipv6`  
North star: `docs/IPV6_NATIVE_P2P_SPEC.md`  
Salvage decisions: `docs/IPV6_SALVAGE_NOTES.md`

## 1. How To Use This Document

This roadmap is the trail map from the current repository to the IPv6-native Voxelle target.

It is intentionally more operational than the specification:

- The spec defines what the system is.
- The salvage notes explain what we kept or rejected from the old implementation.
- This roadmap defines the staged path, gates, task names, and deletion points.
- `.isnad` tracks live work, snapshots, tests, and steering.

Chat can stay lightweight. Durable project memory should land here or in `.isnad`.

## 2. Operating Principles

1. Build from protocol truth upward.
2. Keep each layer testable before the next layer depends on it.
3. Avoid networking until local correctness is boring.
4. Avoid UI until CLI/runtime behavior is inspectable.
5. Delete legacy surfaces only after replacement gates pass.
6. Prefer Rust for the core/runtime unless evidence from this branch says otherwise.
7. Keep implementation commits small enough to review and revert.

## 3. Current State

Completed:

- Greenfield IPv6 native P2P spec.
- Salvage memo.
- `crates/voxelle-core` initial milestone:
  - peer/device key generation
  - Ed25519 SPKI-derived IDs
  - device delegation
  - event signing/validation
  - content-addressed event IDs
  - canonical parent handling
  - DAG heads
  - deterministic topological ordering

Verified:

```text
cargo test -p voxelle-core
cargo test
```

## 4. Gate 1: Core Truth

Task: `ipv6_core_acceptance_pipeline`

Purpose:

Move `voxelle-core` from cryptographic validity to room truth. A signed event is not automatically acceptable; it must be valid under room membership, device authorization, governance state, and local limits.

Required capabilities:

- room genesis event/object
- governance room model
- member join event
- member ban/unban
- device authorization/revocation
- conservative unknown-kind handling
- acceptance pipeline:
  - size/shape limits
  - cryptographic validation
  - governance derivation
  - permission check
  - accepted/rejected result

Required tests:

- non-member message is rejected
- valid `MEMBER_JOIN` admits peer
- admitted member message is accepted
- banned peer cannot post
- revoked device cannot post
- missing ancestors are tolerated for otherwise valid events
- unknown event kind does not bypass permissions
- governance derivation is deterministic from shuffled input

Done means:

```text
cargo test -p voxelle-core
```

passes with acceptance-pipeline tests, and the public API makes it hard to store an event that has not been accepted.

Deletion unlocked:

- Most old frontend protocol helpers become reference-only.

## 5. Gate 2: Local Store

Task: `ipv6_store_sqlite`

Purpose:

Make room durability real without involving a network.

Recommended crate:

```text
crates/voxelle-store
```

Required capabilities:

- initialize local store
- persist peer/device identity material or store references
- insert events idempotently
- load events by room
- load event by ID
- compute or cache room heads
- preserve data across process restarts
- record validation/acceptance status clearly

Required tests:

- inserting same event twice is idempotent
- room events survive reopening the store
- heads are stable after reopen
- invalid/unaccepted event cannot be inserted through the accepted-event API

Done means:

```text
cargo test -p voxelle-store
cargo test
```

passes.

Deletion unlocked:

- Browser localStorage/IndexedDB storage code can be deleted after Gate 2 or Gate 3.

## 6. Gate 3: Local Two-Store Sync

Task: `ipv6_sync_local_two_store`

Purpose:

Prove anti-entropy sync without sockets.

Recommended crate:

```text
crates/voxelle-sync
```

Required capabilities:

- summarize local rooms
- exchange heads
- compute wants
- send bounded event batches
- validate before insert
- converge two independent stores
- expose useful sync stats

Minimum grammar:

```text
HELLO
ROOMS
HEADS
HAVE
WANT
EVENTS
ACK
ERROR
```

Required tests:

- Alice and Bob stores converge from one missing event
- missing ancestor is requested before dependent child is fully useful
- duplicate events are ignored
- invalid forwarded event is rejected
- batch limits are enforced
- three-store forwarding scenario converges without a central relay

Done means:

```text
cargo test -p voxelle-sync
cargo test
```

passes with local in-process sync only.

Deletion unlocked:

- Old WebRTC sync code becomes disposable.

## 7. Gate 4: CLI Runtime

Task: `ipv6_cli_runtime`

Purpose:

Create a usable inspection surface before real networking and before GUI work.

Recommended crate:

```text
crates/voxelle-cli
```

Required commands:

```text
voxelle identity create
voxelle room create
voxelle invite export
voxelle invite import
voxelle event send
voxelle room heads
voxelle sync local
voxelle diagnose
```

Required tests:

- CLI can create identity and room
- CLI can export/import an invite between two stores
- CLI can append a message
- CLI can run local sync and converge two stores

Done means:

The minimum local workflow can be completed from commands without UI or network.

Deletion unlocked:

- Old React UI is no longer needed as a manual protocol driver.

## 8. Gate 5: IPv6 Transport

Task: `ipv6_net_quic_direct`

Purpose:

Add real peer connectivity once local correctness is stable.

Recommended crate:

```text
crates/voxelle-net
```

Required capabilities:

- listen on IPv6
- connect to IPv6 endpoint
- device-authenticated handshake
- protocol version negotiation
- endpoint record exchange
- run sync over transport
- bounded queues and backpressure
- local IPv6 diagnostics
- inbound reachability diagnostics

Required tests:

- loopback IPv6 connection succeeds
- handshake rejects identity mismatch
- sync runs over transport between two local stores
- stale endpoint is diagnosed legibly
- queue/batch/message limits are enforced

Done means:

Two local processes can sync a room over IPv6 without signaling, WebRTC, or Voxelle-operated infrastructure.

Deletion unlocked:

- `crates/voxelle-signal`
- WebRTC transport code
- field-test tunnel scripts

## 9. Gate 6: Legacy Deletion

Task: `ipv6_delete_legacy_stack`

Purpose:

Make the repository structurally match the new target after replacements exist.

Delete candidates:

- `apps/web`
- `apps/desktop`
- old WebRTC code
- old signaling relay
- field-test tunnel docs/scripts
- release/update machinery tied to old app shape

Keep candidates:

- `.isnad`
- `docs/IPV6_NATIVE_P2P_SPEC.md`
- `docs/IPV6_SALVAGE_NOTES.md`
- `docs/IPV6_ROADMAP.md`
- useful protocol docs after reconciliation
- new Rust crates

Done means:

The repo builds/tests around the IPv6-native runtime only.

## 10. Gate 7: Minimal Desktop UI

Task: `ipv6_minimal_desktop_ui`

Purpose:

Add a user surface after the runtime is real.

Possible approaches:

- small native UI over CLI/daemon
- terminal UI
- lightweight local UI process
- defer entirely if CLI is sufficient for the first field test

Required views:

- identity status
- room list
- invite import/export
- messages
- peer reachability
- room health
- diagnostics

Done means:

The UI exposes the runtime honestly without becoming a second protocol implementation.

## 11. Active Task Order

Immediate order:

1. `ipv6_core_acceptance_pipeline`
2. `ipv6_store_sqlite`
3. `ipv6_sync_local_two_store`
4. `ipv6_cli_runtime`
5. `ipv6_net_quic_direct`
6. `ipv6_delete_legacy_stack`
7. `ipv6_minimal_desktop_ui`

The only task currently ready to implement is `ipv6_core_acceptance_pipeline`.

## 12. Check-In Policy

The agent should lead by default:

- choose the next task from this roadmap
- implement until a gate or decision point is reached
- run focused tests
- record snapshots in `.isnad`
- report concise check-ins in chat

Ask for user input when:

- a choice changes product philosophy
- a dependency/runtime choice becomes sticky
- deletion would remove potentially useful reference material before its gate
- implementation evidence contradicts this roadmap

