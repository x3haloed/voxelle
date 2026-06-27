# IPv6 Branch Salvage Notes

Status: working decision memo  
Branch: `ipv6`  
Purpose: decide what, if anything, should survive from the current implementation before building the IPv6-native target.

## 1. Decision

Do not preserve the existing application architecture.

Preserve a small set of protocol decisions, tests, and implementation lessons. Treat the current codebase as reference material, not as a foundation that must be kept alive.

The new target should be built as a CLI/runtime-first native system. The old web, Tauri, WebRTC, signaling, tunnel, and release surfaces can be deleted once the new local protocol base is proven.

## 2. What Survives

### 2.1 Event DAGs

Survives as a core model.

Current references:

- `docs/P2P_INVITE_SPACE_CHAT_RFC.md`
- `apps/web/src/voxelle/dag.ts`
- `apps/web/src/voxelle/events.ts`
- `apps/web/src/voxelle/store.ts`

Useful decisions:

- Events are immutable signed records.
- Rooms are DAGs, not linear logs.
- Events reference parent event IDs.
- Missing ancestors are tolerated temporarily.
- Room heads are event IDs with no known children.
- Display/state derivation uses deterministic topological order.
- Tie-breaking by timestamp and event ID is good enough for v0 display determinism.

Rewrite implication:

Build this fresh in the new core. Do not copy the TypeScript implementation, but port the tests and behavior.

### 2.2 Content-Addressed Event IDs

Survives.

Current references:

- `docs/P2P_INVITE_SPACE_CHAT_RFC.md`
- `apps/web/src/voxelle/rfc/signing.ts`
- `apps/web/src/voxelle/rfc/validate.ts`

Useful decisions:

- Event ID is derived from the canonical signature input, not assigned by a server.
- Signature input excludes `sig` and `event_id`.
- Recomputing the event ID is part of validation.
- Parent IDs are sorted before event signing in current code.

Rewrite implication:

Keep content-addressed event IDs. Decide explicitly whether parent sorting is required by spec. My recommendation is yes: canonicalize parent IDs before signing so logically identical events do not fork due only to parent order.

### 2.3 Identity and Device Split

Survives.

Current references:

- `docs/P2P_INVITE_SPACE_CHAT_RFC.md`
- `apps/web/src/voxelle/rfc/signing.ts`
- `apps/web/src/voxelle/rfc/delegation.ts`
- `crates/voxelle-protocol/src/ids.rs`
- `crates/voxelle-protocol/src/spki_ed25519.rs`

Useful decisions:

- Long-lived peer/principal identity is distinct from device identity.
- Device keys sign day-to-day events.
- Root identity authorizes device keys through delegation certificates.
- IDs are derived from public keys.
- Current Rust and TypeScript use `ed25519:` plus base64url SHA-256 of SPKI DER.

Rewrite implication:

Keep the split. Rename `principal` to `peer` if the new spec prefers that language, but do not collapse peer and device identity. The split is necessary for multi-device use, revocation, and safer key handling.

### 2.4 Canonical Encoding and Signing Inputs

Survives, with possible cleanup.

Current references:

- `docs/P2P_INVITE_SPACE_CHAT_RFC.md`
- `crates/voxelle-protocol/src/jcs.rs`
- `crates/voxelle-protocol/src/netstring.rs`
- `apps/web/src/voxelle/rfc/netstring.ts`
- `apps/web/src/voxelle/rfc/jcs.ts`

Useful decisions:

- Do not sign raw JSON.
- Use domain-separated signing inputs.
- Use netstrings for stable field framing.
- Use JCS for extensible JSON bodies.

Rewrite implication:

Keep this unless there is a strong reason to switch to a binary canonical format. If we switch, make that decision now and define it once. Good candidates:

- Current netstring plus JCS: easiest to salvage and audit.
- CBOR with deterministic encoding: cleaner binary protocol, but more compatibility decisions.

Recommendation: keep netstring plus JCS for milestone 1 because it is already specified and partially implemented in Rust.

### 2.5 Governance State as a Pure Derivation

Survives.

Current references:

- `docs/P2P_INVITE_SPACE_CHAT_RFC.md`
- `apps/web/src/voxelle/governance.ts`
- `apps/web/src/voxelle/accept.ts`

Useful decisions:

- Membership and permissions are protocol state.
- Governance events live in a dedicated governance room.
- Peers derive authorization state from accepted governance events.
- Event acceptance separates cryptographic validation from policy validation.

Rewrite implication:

Build a pure state machine in the new core. It should accept an ordered governance DAG and produce membership/device/role state. Avoid UI dependencies and storage dependencies.

### 2.6 Acceptance Pipeline

Survives as a design pattern.

Current references:

- `apps/web/src/voxelle/accept.ts`
- `apps/web/src/voxelle/rfc/validate.ts`
- `apps/web/src/voxelle/limits.ts`

Useful decisions:

The current pipeline is:

1. Check local size/shape limits.
2. Validate cryptographic structure.
3. Derive governance state.
4. Check membership and bans.
5. Store only accepted events.

Rewrite implication:

This becomes a first-class core API:

```text
validate_event_bytes -> ValidSignedEvent
derive_room_state -> RoomState
accept_event -> Accepted | Rejected(reason)
```

The API should make it hard to store unvalidated events accidentally.

### 2.7 Minimal Anti-Entropy Sync Grammar

Survives conceptually.

Current references:

- `apps/web/src/voxelle/sync.ts`
- `docs/IPV6_NATIVE_P2P_SPEC.md`

Useful decisions:

- Start with heads exchange.
- Request missing event IDs.
- Send bounded batches of events.
- Validate before inserting.
- Track accepted counts.
- Apply message and verification rate limits.

Rewrite implication:

The current code is tied to one WebRTC room session and should be discarded. The new protocol should generalize the grammar to peer sessions:

```text
HELLO
PEER_RECORDS
ROOMS
HEADS
HAVE
WANT
EVENTS
ACK
ERROR
PING
PONG
```

### 2.8 Bounds and Abuse Limits

Survives.

Current references:

- `apps/web/src/voxelle/limits.ts`
- `crates/voxelle-signal/src/main.rs`

Useful decisions:

- Message size limits are mandatory.
- Batch size limits are mandatory.
- Queue limits are mandatory.
- Verification budgets matter because signature checks are attacker-controlled work.
- Session/client caps matter for network code.
- TTLs matter for temporary records.

Rewrite implication:

Every public parser, network handler, sync queue, endpoint cache, and store path should have explicit limits from the first implementation.

### 2.9 Existing Rust Protocol Crate

Partially survives.

Current references:

- `crates/voxelle-protocol`

Useful code:

- Ed25519 SPKI parsing.
- ID derivation.
- JCS wrapper.
- Netstring writer.
- Smoke tests for ID stability and netstring formatting.

Rewrite implication:

This crate can be renamed or replaced by `voxelle-core`. It is small enough to keep as seed code, but not important enough to constrain the architecture.

## 3. What Does Not Survive

### 3.1 Web App

Delete after the new core has a passing local protocol base.

Reasons:

- Browser constraints are explicitly out of scope.
- WebRTC is no longer required.
- IndexedDB/localStorage concerns disappear.
- React state is irrelevant to protocol correctness.

### 3.2 Tauri Shell

Delete after the new CLI/runtime base exists.

Reasons:

- The new target avoids App Store/code-signing-driven architecture.
- Tauri is not needed for native socket access if the runtime is already native.
- Desktop UI should come after the daemon/CLI proves the protocol.

### 3.3 WebRTC Transport

Delete.

Reasons:

- The new target uses native IPv6 transport.
- Browser ICE/STUN/TURN complexity is out of scope.
- WebRTC offer/answer flows should not shape the new protocol.

### 3.4 Signaling Relay

Delete as product architecture.

Reasons:

- Required centralized relay/signaling conflicts with the new priority.
- Its useful lessons are limits, TTLs, and rate-limiting, not the relay itself.

### 3.5 Field-Test Tunnel Scripts

Delete.

Reasons:

- localhost.run/Cloudflare tunnels are workarounds for the old web field-test path.
- The new target should diagnose real IPv6 reachability directly.

### 3.6 Release and Update Machinery

Delete or ignore for now.

Reasons:

- The base target may be source-built, script-based, or package-manager distributed.
- Update delivery is not part of the stable protocol base.

## 4. Tests Worth Porting

### 4.1 Keep/Port

Port these as native/core tests:

- Stable peer ID from the same SPKI DER.
- Ed25519 SPKI parse roundtrip.
- Netstring writer expected byte format.
- Delegation signature verifies and binds peer/device IDs.
- Event signature verifies.
- Event ID recomputes from signature input.
- Event parent canonicalization is deterministic.
- DAG head calculation.
- Deterministic topological sort.
- Governance `MEMBER_JOIN` admits a peer.
- Governance ban prevents later room event acceptance.
- Unknown event kinds validate cryptographically but require conservative permission handling.
- Missing ancestors do not prevent storing otherwise valid events.

### 4.2 Recast as Acceptance Scenarios

The old Playwright test scenario should become a CLI/integration test:

1. Alice creates identity and room.
2. Alice exports invite.
3. Bob imports invite.
4. Bob emits `MEMBER_JOIN`.
5. Alice and Bob exchange events.
6. A message event converges on both stores.

Later, add:

1. Carol joins.
2. Alice goes offline.
3. Bob forwards events to Carol.
4. All three converge.

### 4.3 Do Not Port

Do not port:

- UI tests.
- WebRTC loopback tests.
- Tauri updater tests.
- tunnel launcher checks.

## 5. Language and Runtime Choice

Rust is a strong default, but not a requirement.

### 5.1 Why Rust Fits

Rust is a good fit for the new base because:

- Native networking and filesystem access are first-class.
- `quinn` gives a mature QUIC stack.
- SQLite bindings are solid.
- Type-level separation can protect validation boundaries.
- Static binaries make source/package distribution plausible.
- Existing repo already has Rust protocol primitives.
- Long-running daemon code benefits from memory safety.

### 5.2 Rust Costs

Rust costs:

- Slower iteration than scripting languages.
- More friction for quick UX experiments.
- QUIC/TLS/cert plumbing can get verbose.
- Contributors may face a steeper learning curve.

### 5.3 Reasonable Alternatives

Go is the strongest alternative if the priority becomes operational simplicity and fast iteration.

Pros:

- Simple static binaries.
- Excellent networking ergonomics.
- Good cross-compilation.
- Easier contributor ramp than Rust.
- Mature QUIC library ecosystem.

Cons:

- Less precise type modeling.
- More runtime footguns around validation boundaries.
- Existing Rust salvage becomes reference-only.

Python is useful for prototypes, not for the base daemon.

Pros:

- Fastest design iteration.
- Good for diagnostics experiments.
- Easy scripting distribution for technical users.

Cons:

- Harder secure packaging.
- Weaker long-running daemon story.
- Async networking and binary distribution are more fragile.
- Cryptographic and persistence boundaries require extra discipline.

### 5.4 Recommendation

Use Rust for the protocol/runtime base unless we make a positive decision to optimize for Go's faster implementation loop.

Do not choose Python for the durable base. Python can be used for small diagnostic scripts or test harnesses if helpful.

## 6. Proposed New Repo Shape on This Branch

The new branch should move toward:

```text
crates/voxelle-core      identity, canonical encoding, events, DAG, governance
crates/voxelle-store     SQLite/local durable storage
crates/voxelle-sync      anti-entropy sync state machine
crates/voxelle-net       IPv6 QUIC transport and diagnostics
crates/voxelle-cli       CLI entrypoint for protocol/runtime testing
docs/                    specifications and decision records
```

Short-term, keep `crates/voxelle-protocol` only if it accelerates `voxelle-core`. Otherwise fold its useful code into `voxelle-core` and delete it.

## 7. Deletion Gates

Do not delete old surfaces before these pass:

### Gate 1: Local Core

Required:

- create peer identity
- create device identity
- authorize device
- create room genesis
- append message event
- validate event
- compute room heads
- derive deterministic event order

When Gate 1 passes, delete or ignore most frontend code confidently.

### Gate 2: Invite and Membership

Required:

- export invite
- import invite
- create member join event
- derive membership state
- reject non-member message
- accept member message

When Gate 2 passes, old invite/join implementation is no longer useful.

### Gate 3: Local Two-Store Sync

Required:

- two local stores
- exchange heads
- request missing events
- transfer event batches
- validate before insert
- converge room heads

When Gate 3 passes, old WebRTC sync code is no longer useful.

### Gate 4: IPv6 Transport

Required:

- listen on IPv6
- connect over IPv6
- authenticated handshake
- run sync over transport
- diagnose local IPv6 and inbound reachability

When Gate 4 passes, old signaling/WebRTC/tunnel surfaces can be deleted.

## 8. Immediate Next Step

Start with `voxelle-core` in Rust.

First implementation target:

```text
cargo test -p voxelle-core
```

with tests for:

- identity ID derivation
- device delegation
- event signing
- event validation
- DAG heads
- deterministic topological order

This step is intentionally network-free. If the local protocol is not solid, IPv6 transport will only make failures harder to understand.

