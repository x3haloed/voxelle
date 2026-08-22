# Reticulum as a Future Voxelle Bearer

Status: non-normative architectural consideration  
Decision: deferred; no current product or protocol change  
Recorded: 2026-08-21

## Purpose

This note preserves the analysis of whether the
[Reticulum Network Stack](https://reticulum.network/) could help Voxelle. It is
an option record for future evaluation, not an accepted revision to Voxelle's
contract, protocol, implementation, or evidence claims.

The controlling sources remain, in order:

1. [`TRUTHFUL_SYSTEM_CONTRACT.md`](TRUTHFUL_SYSTEM_CONTRACT.md)
2. [`P2P_INVITE_SPACE_CHAT_RFC.md`](P2P_INVITE_SPACE_CHAT_RFC.md)
3. [`IPV6_NATIVE_P2P_SPEC.md`](IPV6_NATIVE_P2P_SPEC.md)

If this note conflicts with those sources, they control. In particular, direct
IPv6 QUIC, the Rust runtime, the one-process preference, and the current native
product envelope remain binding until an explicit contract revision says
otherwise.

## Summary Assessment

Reticulum is strong architectural prior art for Voxelle and could eventually be
useful as an optional reachability bearer. It should not currently replace
Voxelle's native IPv6 QUIC transport, become a required runtime, or acquire any
identity, governance, admission, storage, recovery, or encryption authority.

The most promising future shape is to carry opaque, already-authenticated
Voxelle protocol envelopes through Reticulum when direct IPv6 reachability is
unavailable. Facts received through that bearer would still pass through the
same Voxelle event-admission pipeline, durable store, synchronization logic, and
projection path used by direct QUIC.

## Why Reticulum Is Relevant

Reticulum makes cryptographic destinations independent of network locations and
can route over heterogeneous carriers such as packet radio, LoRa, serial links,
Wi-Fi, Ethernet, and IP. Its transport nodes forward announces and traffic
without becoming application authorities. It also supports encrypted links,
path discovery, reliable resource transfer, and operation across highly
variable bandwidth and latency.

These properties reinforce several existing Voxelle choices:

- identity must not be derived from an address, hostname, endpoint, or service;
- routing and forwarding may improve availability without granting protocol
  authority;
- ordinary peers may bridge changing paths and heterogeneous links;
- transport should move authenticated facts rather than decide their meaning;
- intermittent and store-and-forward operation deserves explicit treatment;
- ordinary success should hide topology while degraded states expose it
  truthfully.

Primary Reticulum references consulted for this assessment:

- [Reticulum repository and protocol overview](https://github.com/markqvist/Reticulum)
- [Understanding Reticulum](https://reticulum.network/manual/understanding.html)
- [Building Reticulum networks](https://reticulum.network/manual/networks.html)
- [Reticulum API reference](https://reticulum.network/manual/reference.html)
- [LXMF distributed messaging](https://github.com/markqvist/LXMF)

These are external, evolving sources and must be rechecked before any future
decision or implementation.

## Potential Value to Voxelle

### Optional heterogeneous reachability

A Reticulum adapter could provide a continuation path over community meshes,
packet radio, LoRa, serial connections, or Reticulum-over-IP when direct IPv6
QUIC cannot connect. This would extend reachability without changing the
meaning of a Voxelle fact.

### Disruption-tolerance evidence

Reticulum and LXMF provide useful implementation evidence for path discovery,
queued delivery, retries, distributed propagation stores, and later retrieval.
Those mechanisms may inform Voxelle's offline-inviter onboarding, ordinary-peer
retention, and bounded store-and-forward behavior even if Voxelle never embeds
Reticulum.

### Transport-independence test

An optional adapter would test whether Voxelle's event system is genuinely
transport-independent. The same signed fact should be admitted, retained,
replicated, and projected identically whether it arrives over direct QUIC or an
indirect Reticulum path.

## Boundaries That Must Survive

Any future Reticulum work must preserve all existing defended invariants. The
following boundaries are especially relevant.

### Identity and recovery

A Reticulum identity or destination is not a Voxelle principal. It must not
replace principal genesis, the ordered identity log, root rotation, device
authorization, device revocation, recovery capabilities, or encryption
capabilities. If Reticulum authenticates a transport endpoint, Voxelle must
still independently establish the principal and authorized device represented
by the supplied Voxelle identity proof.

### Governance and admission

Reticulum path discovery, successful link establishment, delivery confirmation,
or decryption must never imply space membership, room membership, permission,
or event validity. Every received fact must traverse Voxelle's one admission
truth, including authentication, current device authorization, governance,
creation-time validity, permissions, bans, revocation, parent rules, and input
bounds.

Retries, forwarding, reordering, duplicated delivery, and path changes must not
create a second interpretation of a fact.

### Retention and synchronization

Reticulum or LXMF propagation nodes may at most retain and forward opaque
Voxelle envelopes. They must not become authoritative history, an admission
ledger, a required availability provider, or a parallel messaging model.
Voxelle's accepted signed facts and monotonic identity/governance state remain
authoritative; local SQLite remains the durable retained store and source for
reconstructible projections.

### Private-room confidentiality

Reticulum transport encryption and group destinations must not replace
Voxelle's governance-bound private-room membership, explicit encryption epochs,
or recovery of intended room key material. A forwarding peer may retain private
room ciphertext without receiving decryption or membership authority. After
decryption, Voxelle must still semantically validate the contained fact.

### Live media

Reticulum is not presently a candidate replacement for Voxelle's direct
two-to-four-participant media mesh. Live media needs its own authenticated,
membership-aware, low-latency path and explicit degraded states. A future
Reticulum adapter should initially exclude voice and video.

### Runtime and packaging

The current project envelope permits native Rust, one operating-system WebView,
SQLite, and preferably one ordinary-user process. Reticulum's primary runtime
is Python. Making it required, bundling a second runtime, or introducing a
mandatory daemon would cross the authorized embodiment depth and require an
explicit contract revision plus packaging, integrity, lifecycle, and resource
evidence on all supported platforms.

## Candidate Future Shape

```text
durable principal and authorized device
                 |
        signed Voxelle fact
                 |
       outbound envelope framing
                 |
       +---------+----------+
       |                    |
direct IPv6 QUIC     optional Reticulum bearer
       |             opaque transport only
       +---------+----------+
                 |
       one admission pipeline
                 |
      accepted SQLite history
                 |
       sync and projection
                 |
        observable product UI
```

Surviving authorities would be unchanged:

- the identity genesis and identity log decide principal, root, and device
  truth;
- space governance decides membership, permissions, bans, rooms, invites, and
  encryption membership;
- the Voxelle event-admission pipeline decides which facts are accepted;
- SQLite retains accepted facts and reconstructible projections;
- native Rust command and view identities control the workbench surface;
- scoped live-media participants control ephemeral call state.

Reticulum would decide only how an opaque envelope moves toward a destination.

## Smallest Honest Experiment

Do not begin this experiment until the direct non-loopback IPv6 field test is
solid enough to provide a preservation baseline.

The first experiment should carry one representative, already-signed public
room message between two Voxelle peers over a local Reticulum network:

1. Create and admit the message normally on peer A.
2. Export the existing bounded synchronization envelope without translating it
   into an LXMF chat message or Reticulum governance object.
3. Carry that opaque envelope through Reticulum to peer B.
4. Bind the remote session to the expected Voxelle peer and authorized device;
   do not infer membership from the Reticulum destination.
5. Submit every received fact to the existing Voxelle admission pipeline.
6. Commit accepted facts through the existing SQLite transaction path.
7. Project the accepted message through the real application bridge and UI.
8. Repeat delivery, reorder it, interrupt the path, restart both peers, and
   verify idempotent convergence.
9. Inspect transmitted and retained artifacts to confirm that no unintended
   plaintext, capability, or parallel authoritative state was introduced.

The experiment should initially exclude:

- principal creation, recovery, or root rotation through Reticulum identity;
- private rooms, DMs, or encryption-key distribution;
- invites or governance mutations;
- LXMF-native conversations or propagation history as product truth;
- attachments large enough to obscure the admission-path question;
- live voice or video;
- required background daemons or production packaging.

## Evidence Required Before Adoption

An adoption proposal would need evidence that:

- direct QUIC and Reticulum delivery produce identical admission decisions and
  projections for the same fact;
- unauthenticated, revoked, banned, stale, oversized, reordered, and replayed
  inputs are rejected identically on both paths;
- Reticulum identities and destinations cannot grant Voxelle authority;
- losing every Reticulum transport or propagation node leaves direct Voxelle
  operation intact;
- no Reticulum node is irreplaceable for identity, relationships, retained
  history, recovery, or communication;
- private ciphertext can be forwarded without granting decryption rights;
- restart, local loss, recovery, revocation, and resynchronization retain their
  current meaning;
- process count, memory, storage, bandwidth, latency, installer size, lifecycle,
  and failure behavior are measured separately from engineering convenience;
- macOS Apple Silicon, macOS Intel, and Windows artifacts remain buildable,
  integrity-verifiable, and installable within the unsigned-install contract;
- the packaged native surface exposes truthful progress, degraded states, and
  manual diagnostics without making users manage topology during ordinary
  success.

## Decision Triggers

Reconsider a Reticulum bearer only if one or more of the following becomes
important enough to justify the added boundary:

- field evidence shows direct IPv6 QUIC leaves a material, in-envelope
  reachability gap that ordinary Voxelle forwarding cannot address;
- users need operation across non-IP or extremely disruption-prone links;
- a Rust-compatible integration or bounded sidecar lifecycle can satisfy the
  native packaging and resource envelope;
- a prototype proves transport independence without introducing a second
  authority, store, or messaging workflow;
- the product contract explicitly expands beyond IPv6-native operation.

If adopted, update the truthful-system contract and the IPv6-native
specification before treating the new bearer as preserved product behavior.
Record whether direct IPv6 QUIC remains required, optional, or merely preferred;
do not let implementation presence silently make that decision.

## Current Decision

Reticulum is deferred as a promising optional bearer and retained as design
evidence. Voxelle continues with direct IPv6 QUIC and ordinary-member forwarding
as its current transport contract. No Reticulum dependency, compatibility
layer, runtime, identity mapping, storage path, or user-facing claim is accepted
by this note.
