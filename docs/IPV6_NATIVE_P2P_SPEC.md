# Voxelle IPv6 Native P2P Specification

Status: draft  
Audience: implementers  
Scope: greenfield definition of the simplified Voxelle shape

## 1. Position

Voxelle is a native, local-first, IPv6-only peer-to-peer room system.

The system prioritizes:

1. No Voxelle-operated centralized infrastructure.
2. A stable base that works under explicit network requirements.
3. Durable rooms whose state can survive peer churn, address churn, and intermittent connectivity.
4. Legible failure modes over transparent fallback complexity.

Voxelle is not designed to work on every network. IPv6 support and usable peer reachability are structural requirements of the application.

The product surface can remain minimal until the protocol, runtime, and diagnostics are reliable.

## 2. Core Commitments

### 2.1 Native Only

The reference application is a native desktop application, CLI, daemon, or scriptable runtime.

There is no browser client in the base architecture. This removes WebRTC as a platform requirement and allows the system to use native sockets, QUIC, UDP, TCP, local discovery, filesystem storage, and OS-level diagnostics directly.

Distribution may be unsigned, source-based, script-based, package-manager-based, or user-compiled. App Store distribution is not part of the base requirement.

### 2.2 IPv6 Only

All network transports in the base system require IPv6.

IPv4 support, NAT traversal, TURN, and centralized relay compatibility are non-goals for the base system. Future adapters may exist, but they must not be required for protocol correctness.

### 2.3 No Required Central Services

The base system must not depend on Voxelle-operated servers for:

- identity
- discovery
- signaling
- relay
- storage
- push notification
- room membership
- conflict resolution
- update delivery

Users may choose to run always-on peers, personal servers, NAS devices, VPS nodes, or shared community peers. These are ordinary protocol participants, not privileged infrastructure.

### 2.4 Local-First Truth

Room truth is carried by signed, append-only events. Transport only moves events. Transport does not define authority, ordering, membership, or validity.

Any peer with the relevant room state must be able to validate, store, merge, and forward events without asking a central service.

## 3. Non-Goals

The base system does not attempt to provide:

- universal connectivity
- IPv4 support
- browser support
- App Store distribution
- centralized account recovery
- globally searchable users
- guaranteed online delivery
- guaranteed push notifications
- hidden network failure
- automatic relay through Voxelle infrastructure
- anonymity
- metadata privacy against connected peers
- large public room scalability

These may become future extension areas, but they are not part of the stable base.

## 4. System Model

### 4.1 Peer

A peer is a human, organization, or long-lived logical participant.

Each peer has a stable identity keypair.

```text
peer_id = hash(peer_public_key)
```

The peer identity key signs durable claims such as device authorization, profile metadata, and room membership actions where permitted.

### 4.2 Device

A device is a concrete runtime installation controlled by a peer.

Each device has its own keypair.

```text
device_id = hash(device_public_key)
```

A peer may authorize multiple devices. Device authorization is represented as signed protocol state, not by a server account.

### 4.3 Endpoint

An endpoint is a temporary network reachability claim for a device.

```text
endpoint = {
  device_id,
  transport,
  ipv6_addr,
  port,
  observed_at,
  expires_at,
  reachability,
  signature
}
```

Endpoints are not identity. They are hints. They may expire, rotate, fail, or be superseded.

### 4.4 Room

A room is a replicated signed event log with membership and permissions.

```text
room_id = hash(room_genesis_event)
```

Rooms may represent conversations, spaces, administrative channels, device-management contexts, or durable shared state.

### 4.5 Event

An event is an immutable signed object.

```text
event = {
  version,
  room_id,
  event_id,
  author_peer_id,
  author_device_id,
  kind,
  parents,
  body,
  created_at,
  signature
}

event_id = hash(canonical_event_without_signature)
```

Events form a DAG. Each event references zero or more parent events in the same room.

Implementations must validate:

- canonical encoding
- event hash
- signature
- room membership
- author device authorization
- event kind permissions
- parent references
- body size limits
- protocol version compatibility

## 5. Transport

### 5.1 Preferred Transport

The preferred transport is QUIC over IPv6.

QUIC is preferred because it provides:

- encrypted sessions
- stream multiplexing
- datagram support where useful
- connection migration potential
- better behavior under changing network paths than raw TCP

The base protocol should remain transport-framed so TCP/TLS or other native transports can be added later without changing room semantics.

### 5.2 Transport Role

Transport is responsible for:

- connecting to IPv6 endpoints
- authenticating the remote device key
- negotiating protocol versions
- exchanging peer, device, endpoint, and room summaries
- moving signed events and sync messages
- reporting reachability state

Transport is not responsible for:

- deciding room truth
- trusting unsigned data
- assigning global order
- recovering identity
- enforcing product policy outside protocol validation

### 5.3 Connection Handshake

A connection handshake must establish:

1. Remote device public key.
2. Proof that the remote controls the device private key.
3. Supported protocol versions.
4. Supported transports and sync capabilities.
5. Known peer identity bindings for the device.
6. Shared room intersection, if any.

The handshake must not require a central authority.

## 6. Discovery

Discovery is intentionally plural and non-central.

### 6.1 Manual Invites

Manual invite exchange is the required baseline.

An invite may be represented as a file, text blob, QR code, removable-media payload, or copy/paste string.

An invite should contain:

```text
invite = {
  version,
  room_id,
  inviter_peer_id,
  inviter_device_id,
  room_intro_event_or_pointer,
  peer_public_key,
  device_public_key,
  endpoints,
  invite_permissions,
  expires_at,
  signature
}
```

Invites may contain current endpoint hints, but must remain useful if those endpoints are stale. A stale invite can still introduce identity and room material; endpoint refresh can happen later through other peers.

### 6.2 Local Network Discovery

Implementations may support LAN discovery over IPv6 multicast or mDNS.

LAN discovery must only advertise limited device reachability data and must not leak room membership unless the user opts in or the room explicitly permits it.

### 6.3 Peer Exchange

Connected peers exchange known endpoint records for peers/devices they are authorized to discuss.

Endpoint gossip must be bounded, expiring, signed where possible, and treated as hints.

### 6.4 User-Operated Stable Peers

Users may designate always-on peers for availability. Examples include:

- home desktop
- NAS
- VPS
- community server
- shared group machine

These peers have no special protocol authority. They are valuable because they are reachable and store room state.

## 7. Reachability Diagnostics

Reachability is a first-class part of the system.

The application must expose diagnostics that distinguish at least:

- IPv6 unavailable
- IPv6 available
- no usable global IPv6 address
- outbound IPv6 works
- inbound IPv6 blocked
- configured listen port unavailable
- firewall likely blocking inbound traffic
- address changed since last announcement
- endpoint expired
- remote endpoint unreachable
- remote identity verified
- remote identity mismatch
- reachable through one or more room peers

The system should avoid vague connection labels. A user should be able to understand whether a failure is due to local IPv6, local firewall, remote firewall, stale endpoint data, room membership, or protocol validation.

### 7.1 Health States

Room health should be calculated from available peers and known forwarding capacity.

```text
0 reachable forwarders: stalled when local peer is offline
1 reachable forwarder: fragile
2-3 reachable forwarders: healthy for small groups
4+ reachable forwarders: resilient for small groups
```

Health states are advisory. They do not change protocol truth.

## 8. Sync and Forwarding

### 8.1 Sync Model

The base sync protocol is anti-entropy over room DAGs.

When two peers connect:

1. Exchange protocol capabilities.
2. Exchange room membership intersection.
3. Exchange room heads.
4. Compare known event sets or compact summaries.
5. Request missing ancestors.
6. Transfer bounded event batches.
7. Validate before storing.
8. Update local heads.
9. Forward valid events to other eligible peers.

### 8.2 Message Types

The base sync protocol should define messages equivalent to:

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

The exact wire encoding is implementation-defined until stabilized, but all messages must have size limits and versioning.

### 8.3 Forwarding

Peers may forward valid room events authored by other peers.

Forwarding rules:

- Never forward events that fail validation.
- Never forward room events to peers not authorized for that room.
- Preserve original event signatures.
- Do not rewrite event authorship.
- Bound queue sizes.
- Prefer missing events over already-known events.
- Apply backpressure.
- Track repeated failures.

Forwarding is the replacement for centralized relay in the base system.

### 8.4 Store-and-Forward

Any peer may act as a store-and-forward node for rooms it belongs to.

Store-and-forward is ordinary participation:

- The peer stores room events locally.
- The peer advertises reachability.
- The peer syncs with room members.
- The peer forwards missing valid events when asked.

The protocol must not require the store-and-forward peer to be trusted beyond normal room membership permissions.

## 9. Durability

### 9.1 Local Storage

Each device stores:

- peer identity material
- device identity material
- authorized peer/device bindings
- rooms
- events
- room heads
- endpoint records
- sync cursors
- diagnostics history

Storage must be crash-safe. Event insertion should be idempotent.

### 9.2 Event Retention

The stable base should assume full event retention per joined room unless a room policy says otherwise.

Partial retention and compaction are future features. They require checkpointing rules and must not compromise validation of current room state.

### 9.3 Offline Behavior

A peer can create local events while offline if its current room permissions allow it.

Those events become visible to others after a successful sync path exists.

The system does not guarantee delivery time.

### 9.4 Convergence

Given:

- valid room events
- eventual network paths between room members
- no permanent storage loss for all copies of an event

Peers should converge on the same room DAG and derived room state.

## 10. Membership and Permissions

Room membership is protocol state.

The base system should support at least:

- room genesis
- member invite
- member join
- member removal
- device authorization
- device revocation
- role assignment
- permission checks per event kind

Membership changes are signed events. Validation of later events depends on the derived membership state at the event's position in the DAG.

If concurrent governance events conflict, the room specification must define deterministic resolution rules before product use.

## 11. Security Model

### 11.1 Identity

Identity is cryptographic, not account-based.

Losing all identity key material may mean losing the ability to prove continuity. Recovery mechanisms must be explicit protocol features, not server-side assumptions.

### 11.2 Trust

Peers trust:

- their own local keys
- room genesis they accepted
- membership events valid under room rules
- signatures valid under authorized keys

Peers do not trust:

- endpoint claims merely because they were received
- transport peers before handshake verification
- forwarded events before validation
- clocks for authority
- network location as identity

### 11.3 Abuse Resistance

The base implementation must enforce:

- maximum event size
- maximum message size
- maximum batch size
- maximum queue depth
- maximum rooms advertised per handshake
- maximum endpoints per peer/device
- endpoint expiry
- connection rate limits
- validation before storage
- validation before forwarding

### 11.4 Privacy Limits

The base system does not promise anonymity. Connected peers may learn:

- IP addresses
- device reachability
- coarse online status
- room co-membership where authorized
- message timing

The UI and documentation should not imply stronger privacy than the protocol provides.

## 12. User-Visible Model

The user should see network reality clearly.

Important concepts:

- This device's IPv6 status.
- This device's inbound reachability.
- Known devices for each peer.
- Which room members are currently reachable.
- Which peers can forward room data.
- Whether a room is healthy, fragile, or stalled.
- When the local address changed.
- When an invite has stale endpoints but valid identity data.

The application should prefer precise, actionable messages over generic connection failures.

## 13. Implementation Shape

The first implementation should be a protocol/runtime spike, not a full application.

Recommended components:

```text
voxelle-core      event model, validation, room state, canonical encoding
voxelle-store     local durable storage
voxelle-net       IPv6 QUIC transport, handshake, reachability checks
voxelle-sync      anti-entropy sync and forwarding
voxelle-cli       create identity, create room, invite, connect, diagnose, sync
voxelle-daemon    optional long-running peer process
voxelle-ui        later desktop UI over daemon/core
```

The CLI and daemon should be sufficient to prove the system before a UI exists.

## 14. Minimum Working Base

The first stable base is complete when the following scenario works without Voxelle-operated infrastructure:

1. Alice creates a peer identity and room.
2. Alice exports an invite.
3. Bob imports the invite.
4. Alice and Bob connect over IPv6 QUIC.
5. Both verify each other's device identity.
6. Alice sends a room event.
7. Bob receives, validates, stores, and renders it.
8. Bob goes offline and creates an event.
9. Bob reconnects and syncs.
10. Both converge on the same room heads.
11. Carol joins from an invite.
12. Alice goes offline.
13. Bob forwards missing room events to Carol.
14. Diagnostics clearly report whether each device is inbound reachable.

No web app, signaling server, relay server, account server, or Voxelle-hosted service may be required for this scenario.

## 15. Milestones

### Milestone 1: Local Protocol

- identity generation
- room genesis
- canonical event encoding
- event signing
- event validation
- local room DAG storage
- deterministic head calculation

### Milestone 2: Manual Invites

- invite export
- invite import
- room intro
- peer/device records
- endpoint records
- stale endpoint handling

### Milestone 3: IPv6 Direct Transport

- IPv6 listener
- QUIC connection
- device-authenticated handshake
- protocol version negotiation
- basic connection diagnostics

### Milestone 4: Two-Peer Sync

- room intersection
- heads exchange
- want/have/event transfer
- idempotent event insertion
- reconnect and converge

### Milestone 5: Forwarding

- third-peer room join
- event forwarding
- store-and-forward behavior
- bounded queues and backpressure
- room health calculation

### Milestone 6: Diagnostics

- local IPv6 probe
- inbound reachability probe
- firewall diagnosis
- stale endpoint reporting
- room health reporting
- clear CLI output

### Milestone 7: Minimal Desktop Surface

- identity status
- room list
- invite import/export
- message view
- peer reachability view
- room health view

## 16. Design Rules

1. No protocol rule may depend on a Voxelle-operated server.
2. IPv6 failure must be diagnosed, not hidden.
3. Addresses are hints; keys are identity.
4. Transport moves signed facts; it does not create facts.
5. Forwarding must preserve authorship.
6. Every network input must be bounded.
7. Every stored event must be independently validatable.
8. Room convergence matters more than real-time delivery.
9. The CLI must be able to exercise the protocol without a GUI.
10. The base must remain understandable enough to recover from the specification alone.

## 17. Open Questions

These questions must be answered before a production-quality room model:

- Which canonical encoding is used for signed objects?
- Which signature scheme is used for peer and device keys?
- Which QUIC library/runtime is preferred?
- How are concurrent governance events resolved?
- How are device revocations applied to events created around the revocation boundary?
- How should private message contents be encrypted within room events?
- What is the minimum supported backup/export format for identity keys?
- How should a room checkpoint or compaction event be validated?
- What LAN discovery data is safe to advertise by default?
- Whether inbound reachability testing can be purely peer-assisted or needs a user-selected test peer.

