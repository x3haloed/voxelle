# Voxelle Truthful System Contract

Status: locally verified complete through the scoped Discord feature families
Scope: implementation through the Discord feature families  
Governing method: truthful system construction

## Central Truth

A person creates and recovers a durable principal locally; authorized devices
propose signed facts; room members independently validate, retain, replicate,
and project those facts without a privileged service.

A fresh native installation can use a signed invite to join a small private
space, communicate through available ordinary peers, lose its local state,
recover onto a new device, revoke the lost device, resynchronize retained data,
and continue through the same authorities used during normal operation.

No single externally administered provider may be a protocol authority or an
irreplaceable dependency for a person's identity, relationships, retained
history, recovery, or communication.

## Capability And Envelope

The target is a native macOS and Windows application for private groups of 2 to
50 members. The representative lived tests use two or three real processes and
the representative live-media test uses two to four participants.

The completed product slice includes:

- immutable principal identity with root rotation, device authorization and
  revocation, a recovery card, optional guardian recovery policy, encrypted
  recovery capsules, and recovery health;
- signed space invites with space genesis, permissions, expiry, multiple
  bootstrap peers, automatic service launch, join, synchronization, and clear
  disconnected recovery;
- direct IPv6 QUIC plus ordinary-member routing and bounded store-and-forward;
- spaces, channels, profiles, roles, permissions, bans, invites, message posts,
  edits, redaction tombstones, reactions, mentions, threads, pins, unread state,
  notifications, content-addressed attachments, and local full-text search;
- end-to-end encrypted DMs and private channels with membership-bound key
  epochs and recovery of current key material;
- real small-group voice and video carried peer to peer in a two-to-four member
  mesh, with explicit unavailable/degraded states;
- a native Tauri workbench in which every named view surface is dockable, its
  layout survives restart, and one command registry drives buttons, menus,
  shortcuts, the command palette, and the automation surface;
- unsigned but integrity-verifiable artifacts with honest operating-system
  trust instructions. Paid Apple or Microsoft developer accounts are excluded.

## Accepted Revisions And Exclusions

The feature model is Discord-like; Discord scale and centralized availability
are not claimed. The scoped system does not claim:

- large public communities, a global directory, global username uniqueness, or
  public content discovery;
- universal connectivity, guaranteed online delivery, or guaranteed push
  notifications when no member is reachable;
- more than four live-media participants, an SFU, screen sharing, recording,
  or broadcast streaming;
- erasure of plaintext or ciphertext already retained by another participant;
- strict single-use invite counting across a partition;
- warning-free unsigned launch on every macOS or Windows security policy;
- browser participation in the P2P protocol;
- mobile clients or background mobile push.

## Trust And Authority

No Voxelle-operated service is required or authoritative for identity,
membership, permissions, message validity, recovery, discovery, routing,
storage, synchronization, encryption keys, or update integrity.

Ordinary peers may be always-on, route traffic, retain encrypted recovery
capsules, store room history, or forward media. Those roles grant availability,
not protocol authority. Release hosting and a static invite helper may be
optional conveniences; signed artifacts and complete invite payloads must be
mirrorable.

Optional providers may improve discovery, reachability, retention, recovery
availability, notifications, or distribution only when they remain untrusted,
replaceable, non-exclusive, and unable to change protocol identity or
governance. Losing any one provider may reduce availability but must leave a
complete continuation path through another provider or ordinary peers.

Within that boundary, Voxelle pursues two subordinate product values:

- **Progressive topology:** ordinary successful use does not require people to
  understand addresses, peer records, forwarding, synchronization, or provider
  selection. The app automates them while exposing truthful degraded states and
  manual control when intervention is needed.
- **Plural availability:** the runtime uses every authorized ordinary peer and
  replaceable provider that can improve reachability, retention, or recovery
  without granting additional protocol authority.

The surviving authorities are:

- identity genesis and its signed identity log for principal/root/device truth;
- each space governance log for membership, roles, room definitions, bans, and
  invite revocation;
- the event acceptance pipeline for facts admitted to durable storage;
- the local SQLite store for retained accepted facts, validated local device
  state, and reconstructible projections;
- the frontend workbench for ephemeral layout geometry, using stable Rust-owned
  view and command identities;
- the live-media session participants for ephemeral call state.

## Causal Claims

Semantic completion requires the intended domain transition to be accepted by
the named authority, not merely serialized or displayed.

Operational completion requires proportionate evidence for restart,
idempotence, retry, disconnection, partition, stale endpoint, invalid
authorization, revocation, bounded input, loss, and recovery paths.

Lived completion requires the packaged native artifact to accept real keyboard,
pointer, file, clipboard, camera, and microphone inputs that are in scope and
to project the returned accepted facts through the real bridge. Standalone
fixture mutations are not product evidence.

## Topology Preservation

The current Rust authorities for protocol acceptance, SQLite retention, sync,
QUIC transport, the application command host, semantic command IDs, and
semantic view IDs remain unless a complete verified collapse replaces them.
The encrypted identity vault and persistent QUIC credential remain separate
from SQLite because they carry distinct unlock and transport capabilities.

New features extend the existing signed-event, accepted-store, sync, snapshot,
and semantic-command paths. They must not introduce parallel protocol models in
the frontend, preview fixture, CLI, media layer, or recovery UI.

Existing development homes and accepted events are disposable. The new stable
identity and event formats become the first supported product formats; no
legacy reader, migrator, or compatibility authority is retained.

## Embodiment Depth

Implementation may replace algorithms, representations, persistence schemas,
frontend organization, protocol framing, process boundaries, and libraries. It
may collapse the desktop and optional sidecar when one process can carry the
same lifecycle truth.

The admissible runtime remains native Rust plus one operating-system WebView and
SQLite. Separate platform artifacts are allowed. Custom databases, separate
mandatory daemons, bundled browser runtimes, hosted services, ISA-specific
code, custom allocators, kernel components, and specialized hardware are
outside the authorized implementation depth unless measured evidence later
shows the current substrate cannot meet this contract.

The physically smaller theoretical embodiment is a platform-specific native UI
and specialized store without a WebView or SQLite. It is not currently
project-admissible because it would duplicate macOS/Windows UI implementations
and discard the established customizable workbench path.

## Project Constraints

- Keep the core/runtime in Rust and retain independently testable CLI behavior.
- Preserve macOS Apple Silicon, macOS Intel compilation, and Windows dependency
  resolution; run native Windows lived tests when a Windows runner is available.
- Do not require paid Apple or Microsoft developer accounts.
- Prefer one native process for ordinary human use; an optional headless peer
  must reuse the same application authority.
- Do not silently overwrite user changes in a dirty worktree.
- Checkpoint coherent, independently understandable slices with commits,
  provenance snapshots, and pushes to the active branch.

## Evidence Horizon

Locally inspectable evidence includes Rust unit/integration tests, Node UI
behavior tests, clean builds, generated contracts, SQLite and artifact
inspection, multiple loopback IPv6 processes, the packaged macOS artifact,
browser automation against the real Tauri bridge when supported, and local
camera/microphone devices when permission is available.

Windows runtime policy, real non-loopback networks, diverse firewalls, multiple
physical cameras, assistive-technology combinations, and operating-system
unsigned-install policy variants require external runners or machines. Claims
cover them only after those artifacts or runtimes are actually exercised.

## Construction And Verification Order

1. Carry one identity through loss, recovery, rotation, revocation, remote
   synchronization, and visible projection.
2. Carry one signed invite from a fresh packaged launch through automatic join,
   sync, and a visible accepted message.
3. Repeat the join with the inviter offline through an ordinary forwarding peer.
4. Carry existing views and commands through docking, layout restart, palette,
   shortcuts, and the real application bridge.
5. Add one complete Discord feature family at a time, remap the authorities,
   compress the proven path, and rerun preservation evidence.
6. Run the final multi-participant, restart, partition, invalid-input, recovery,
   packaged-artifact, UI, accessibility, resource, clean-build, and cross-target
   gates before making a completion claim.

## Current Risk Frontier

The identity-recovery risk has crossed its first operational gate. Principal
IDs now derive from a self-signed genesis, delegations carry an ordered identity
proof, recovery rotates the root and revokes every old device, and SQLite
retains a monotonic identity head. A two-peer QUIC test carries history from a
lost home through an ordinary retaining peer into a fresh home, propagates the
new head back, and proves that the retaining peer rejects a newly signed event
from the lost device.

Long-lived identity secrets are authenticated ciphertext. Release builds use
macOS Keychain or Windows Credential Manager for the independent unlock key;
unit tests and explicitly opted-in debug CLI tests use a permission-restricted
file key and are not release evidence. The exported `.voxrecover` file is a
permission-restricted bearer recovery capability: possession is sufficient to
rotate the identity, so the UI must tell the person to keep it offline. Its
capsule is independently authenticated and encrypted, and can later be retained
separately by ordinary peers or guardian shares.

Admission and ordinary-peer forwarding have crossed their first operational
gate. Space genesis and expiring invitations are signed governance events;
bootstrap endpoints are covered by the invitation signature; endpoint JSON by
itself grants no membership. A fresh home imports one invite, creates its local
identity, joins, starts service, pushes membership, and pulls history. A
three-home test takes the inviter offline and carries the same join and later
messages through an ordinary member before the authority catches up. Online
shell refresh and message send run bounded concurrent anti-entropy, so ordinary
use does not require manual diagnose/sync choreography. Retained events are
validated at their signed creation time so delegation expiry cannot erase
history; events more than five minutes in the future are rejected. Invite
expiry therefore depends on signed participant clocks and is not claimed as a
partition-proof lease.

Room anti-entropy now exchanges the bounded DAG heads of the requested room
instead of serializing every known event ID or offering an arbitrary prefix of
local history. Each authenticated QUIC exchange computes the causally missing
suffix behind the remote heads, transfers it oldest-first under explicit head
and event limits, then repeats the same authorized path in the reciprocal
direction. A divergent two-peer test converges both stores across multiple
small batches, while a shared-head test sends no duplicate accepted facts.
Membership and private-room authorization are checked before either side
accepts a reciprocal push; the event acceptance authority remains the only
durable admission path.

The unsigned install path has also crossed a native artifact gate: Tauri emits
an ad-hoc-signed universal macOS DMG (arm64 plus x86_64) and is configured for a
Windows NSIS artifact, with SHA-256 manifests and narrow per-app Gatekeeper and
SmartScreen instructions. A native Windows build and first launch still require
the external Windows runner named in the evidence horizon.

The workbench risk has crossed its first lived gate. Every Rust-registered view
can move among all five docks by drag/drop or an accessible selector, reorder,
hide, restore, and reset. Rust rejects incomplete, duplicate, unknown, or
non-contiguous layouts and persists accepted placements in the same preference
state carried by recovery. The command palette, buttons, shortcut matching, and
shell command discovery consume the Rust-owned command registry. A packaged
Tauri run moved and hid views, filtered and invoked the palette from the native
keyboard shortcut, quit, relaunched, and projected the saved layout through the
real bridge.

The public Discord families have crossed their operational gate. Space
governance now carries channel definitions, profiles, roles, permissions,
bans, and invites. Signed room events carry posts, edits, redaction tombstones,
reactions, mentions, threads, pins, and content-addressed attachments. SQLite
retains the accepted facts; multi-room QUIC anti-entropy forwards them through
ordinary peers; the app projects unread cursors, mention notifications, and
local full-text search. A serialized two-home test exercises those commands
through the same host used by the native shell.

Private channels have crossed their first confidentiality and recovery gate.
Each member publishes a signed X25519 encryption key, private channel creation
wraps a random epoch key independently to each admitted member, and every
private room fact is carried inside an authenticated encrypted envelope. A
decrypted inner fact is revalidated by the ordinary semantic authority before
projection. A three-home test proves that the excluded peer neither lists nor
stores the private room, retained events and local key files lack the message
plaintext, admitted peers decrypt successive epochs, and a fresh recovered
home restores the epoch keys and history from an ordinary peer.

The local durability topology now uses one SQLite database for accepted events,
monotonic identity heads, the selected accepted space-genesis ID, peer records,
read cursors, UI preferences, and authenticated ciphertext room-key envelopes.
Space identity, authority, and default-room meaning are reconstructed from that
admitted signed genesis fact rather than copied into a home-config authority.
The previous parallel JSON files and the unconsumed rolling recovery-capsule
cache are gone.
Recovery export derives a fresh authenticated capsule from those authoritative
rows on demand; identity secrets remain in the independently unlocked encrypted
vault and QUIC credentials remain a separate transport capability. A fresh
initialized home therefore materializes only those two capability files plus
SQLite and its crash-safety sidecars.

The small-group media family has crossed its protocol and UI gates. Signed
room-call events carry only participant presence and bounded offer, answer,
and ICE signaling; audio and video remain in direct WebRTC connections with no
STUN, TURN, SFU, or Voxelle service. Concurrent joins converge to the same
deterministically selected four peers without rejecting otherwise valid facts,
and heartbeat expiry releases a slot after a crash. Public and private rooms
reuse the same acceptance, encryption, storage, and sync path. Deterministic UI
tests exercise camera requests, voice fallback when hardware is absent, and
permission-denial truthfulness. This verification machine has no physical
camera or microphone, so physical-device capture is not claimed as lived local
evidence.

The final local fixed-point pass is green for the changed system authorities:
71 Rust tests, 9 browser-shell behavior tests, strict lint, generated-contract
equality, a universal macOS package build, ad-hoc signature and checksum
inspection, packaged native initialization, accepted message projection,
layout and message persistence across restart, IPv6 QUIC startup, and retained
artifact inspection all pass. The universal app contains both arm64 and
x86_64 executables, occupies 31 MiB unpacked, and ships in an 11 MiB DMG. The
initialized native test home occupied 84 KiB after one message and one saved
layout change; its application state consisted of the encrypted identity vault,
the separate QUIC credential, SQLite, and SQLite crash-safety sidecars.

The locally natural compression point is therefore the current authority
topology, not the smallest imaginable byte count. The remaining separations
carry named product meaning:

- collapsing the encrypted principal vault into the QUIC credential would
  merge identity-root, recovery, device, and transport capabilities;
- collapsing the optional headless inhabitant into the desktop would remove
  its independent lifecycle, while making it a mandatory daemon would violate
  the one-process ordinary-use envelope;
- moving call signaling out of retained signed events or pruning it would
  revise the current media acceptance, recovery, and full-retention contract;
- replacing the WebView or SQLite with platform-specific UI or a specialized
  store is beyond the admitted embodiment depth; and
- adding another event encoding or compressed store lacks measured evidence of
  a net resource win at the current 2--50-person envelope and would add a
  parallel representation before it removes one.

Those are bounded revision or evidence questions, not unfinished local
collapses. Workspace-wide formatting and lint are not claimed as green because
the separate provenance-board crates have existing rustfmt drift and two new
`manual_contains` warnings under Rust 1.96; the changed Voxelle authority crates
are formatted and strict-lint clean. Windows first launch, non-loopback field
reachability, and physical media devices remain the external gates already
named in the evidence horizon, not locally completed claims.
