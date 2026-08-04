## Work Board (required)
Before taking meaningful actions, attach to the repo’s provenance system:
1) If `.isnad/` is missing, create it and append an `init` record.
2) Regenerate `.isnad/state/board.md` from `.isnad/ledger.jsonl` + `.isnad/control.jsonl`.
3) Read pending directives in `.isnad/control.jsonl` (any without a matching `ack_directive` in the ledger).
4) Read the latest snapshots for active tasks.
5) Append a `snapshot` (or `resume`) ledger record stating your current understanding and next decision.
Then proceed; acknowledge every new directive with `ack_directive`.

## Local skills
This repo includes local skills under `skills/`. If a user asks to execute a skill, read its `SKILL.md` and follow its “MUST do” requirements.

## Defended system invariants and compressible variants (required)

### Interpretation and document precedence

Defend the system's meaning, authority allocation, security properties, causal
user workflows, and explicit product envelope. Do not defend a current crate,
file, schema, library, process, timer, or protocol mechanism merely because it
exists today.

Use these sources in this order:

1. [The truthful system contract](docs/TRUTHFUL_SYSTEM_CONTRACT.md) is the
   current preservation envelope, evidence horizon, embodiment-depth contract,
   and project-constraint ledger.
2. [The P2P RFC](docs/P2P_INVITE_SPACE_CHAT_RFC.md) specifies identity,
   governance, event, invite, synchronization, and confidentiality semantics in
   more detail. Treat its normative wire and cryptographic choices as protocol
   contracts until an explicit revision replaces them.
3. [The IPv6-native P2P specification](docs/IPV6_NATIVE_P2P_SPEC.md) specifies
   native operation, transport roles, reachability, forwarding, durability,
   and local-first behavior.
4. [The UI ontology](docs/UI_ONTOLOGY.md),
   [unsigned-install guide](docs/INSTALL_UNSIGNED.md),
   [field test](docs/FIELD_TEST.md), and
   [inhabitant surface](docs/inhabitant-surface-v0.md) specify surface-specific
   obligations and evidence.

If documents disagree, do not synthesize a convenient approximation. Preserve
the latest evidence-backed behavior, identify the conflict, and require an
explicit contract decision. Capability is invariant unless the contract records
an accepted revision.

The P2P RFC and IPv6 specification are drafts. Where either conflicts with the
current truthful-system contract and verified merged behavior—especially stable
principal continuity across root rotation—the contract and current evidence
control. Update the draft before relying on its older shape.

### Central truth

A durable person-level principal authorizes devices. Authorized devices propose
authenticated facts. Space members independently validate, retain, replicate,
and project accepted facts without a privileged Voxelle service. A fresh native
installation can join through a signed invite, operate through ordinary peers,
recover after local loss, revoke lost devices, resynchronize retained history,
and continue through the same authorities used during normal operation.

Before a material compression, restate which part of this truth is being
preserved and identify the evidence that will prove it after the collapse. See
the contract's **Central Truth**, **Causal Claims**, and **Topology
Preservation** sections.

### Hard system invariants

These properties must survive compression unless an accepted revision changes
the product contract:

1. **Identity continuity and capability separation.** A principal is not a
   device, key file, hostname, or service account. Devices are independently
   authorized and revocable. Recovery preserves the principal while rotating
   authority away from lost devices. Principal-root, device, recovery, and
   encryption capabilities must not be collapsed into one operating secret.
   See the contract's identity and recovery claims and P2P RFC §5.
2. **Decentralized authority.** No required Voxelle service decides identity,
   membership, permissions, message validity, recovery, encryption keys,
   storage, routing, or update integrity. Ordinary peers may add availability
   without acquiring protocol authority. Endpoint or bootstrap data never
   grants membership. See the contract's **Trust And Authority**, IPv6 spec
   §2.3, and P2P RFC §§1–3.
3. **One admission truth.** Untrusted facts are authenticated and semantically
   validated before durable admission. Membership, roles, bans, device
   revocation, room policy, input bounds, and creation-time validity pass
   through one intelligible decision path. Retries, forwarding, reordering,
   and replay must not create another interpretation. See P2P RFC §§6.4 and
   7.5–7.6 and the contract's **Topology Preservation**.
4. **Governance meaning.** Each space has one deterministic governance truth
   for membership, room definitions, private membership, roles, permissions,
   bans, invitations, revocation, and encryption membership. The representation
   may change; the allocation of decision rights may not. See P2P RFC §6.
5. **Durable convergence.** Accepted facts survive restart, can be recovered
   from ordinary retaining peers, and converge under missing history,
   idempotent retries, ordering differences, and ordinary partitions within
   the stated envelope. Revoked devices cannot restore authority by replay.
   Projections remain reconstructible from authoritative retained meaning. See
   IPv6 spec §§8–9 and P2P RFC §§7.2 and 7.5.
6. **Private-room confidentiality and validation.** Excluded peers cannot
   decrypt private-room content. Key access follows admitted membership and
   explicit epochs. Admitted peers validate decrypted semantic facts; successful
   decryption alone is not authorization. Recovery restores the intended key
   material without creating a recovery service authority. See the contract's
   private-channel claims and P2P RFC §11.
7. **Complete user causal paths.** Fresh setup, signed-invite onboarding,
   offline-inviter onboarding through an ordinary peer, communication, local
   loss, recovery, revocation, resynchronization, and continued operation remain
   complete real paths. Discord-like feature families use the same authority,
   retention, synchronization, and projection story. Failures and unavailable
   states remain explicit. See the contract's **Capability And Envelope** and
   **Construction And Verification Order**.
8. **UI ontology and authority boundary.** Every named view remains dockable;
   placement and visibility survive restart; and one semantic command vocabulary
   drives buttons, shortcuts, the palette, and automation. The frontend does not
   reconstruct protocol authority. Stable IDs must remain stable once persisted
   layouts or external automation depend on them. See UI ontology §§2–5 and the
   inhabitant surface §§1–3.
9. **Live-media boundary.** Media remains direct among the scoped two-to-four
   participants. Participation and signaling are authenticated and
   membership-aware; no relay becomes media or room authority; crash,
   missing-device, and degraded states are explicit; and the four-peer
   projection converges deterministically. See the contract's media claims and
   exclusions.
10. **Install and integrity story.** Native macOS and Windows artifacts remain
    integrity-verifiable and installable without requiring paid Apple or
    Microsoft developer accounts. Trust instructions must remain narrow and
    honest. See the unsigned-install guide and the contract's project
    constraints.
11. **Claim/evidence honesty.** Semantic, operational, and lived claims require
    proportional evidence from the real authority and consumer surfaces.
    Fixtures, types, serialization, intermediate acceptance, or polished pixels
    cannot substitute for an unexercised causal path. Preserve the bounded gaps
    and external gates in the contract's **Evidence Horizon**.

### Negotiated constraints: binding, but revisable

The following are not timeless product truths, but they remain binding until an
explicit contract revision changes them:

- private groups of roughly 2–50 and direct media groups of 2–4;
- direct IPv6 QUIC operation and ordinary-member forwarding;
- native macOS Apple Silicon, macOS Intel, and Windows artifacts;
- a Rust core/runtime, an independently testable CLI, one OS WebView, SQLite,
  and a preference for one ordinary-user process;
- no paid vendor-signing-account requirement;
- the explicit exclusions in the contract, including no claim of centralized
  Discord scale, global discovery, universal delivery, large calls, mobile
  clients, or browser protocol participation.

Do not silently compress one of these away. If a candidate needs a different
envelope or a deeper embodiment, record the accepted revision and update the
relevant documents before treating the change as preservation.

The app is not deployed. Existing development homes and accepted events are
disposable under the contract. Do not add migration readers, legacy writers,
compatibility authorities, or dual workflows unless deployment status or an
explicit user instruction changes this rule.

### Compressible variants

Subject to the negotiated embodiment depth, these are candidates, not sacred
architecture:

- crate, module, binary, and process boundaries;
- Tauri, Quinn, Axum, rusqlite, serde, ts-rs, and other library choices;
- file names and the current split among SQLite and JSON home-state files;
- frontend organization, the full-snapshot bridge, polling loops, heartbeat
  intervals, and scheduler placement;
- command dispatch switches, DTO adapters, generated TypeScript, fixtures, CLI
  adapters, HTTP routes, and button wiring;
- the precise DAG exchange algorithm, known-event representation, batching,
  proof repetition, serialization width, and framing;
- attachment embedding, local indexing, and reconstructible projection layout;
- the current private-event inner/outer envelope and key-package representation;
- the optional inhabitant sidecar and low-level protocol CLI commands when no
  external consumer requires their boundary;
- release container details such as DMG/NSIS and checksum tooling, provided the
  install and integrity invariant remains true;
- specific tests and probes, provided equal or stronger preservation evidence
  replaces them.

Normative cryptographic primitives, IDs, signed encodings, and wire contracts in
the P2P RFC are variants only through an explicit protocol revision with
equivalent security, convergence, interoperability, and recovery evidence. Do
not change them as an incidental refactor.

### High-value compression candidates

Prefer coherent collapses that remove complete authorities, representations,
boundaries, schedulers, or categories of machine work. Current high-value
candidates are:

1. Move call heartbeat, offer, answer, and ICE traffic out of the durable room
   log into an authenticated ephemeral room-signaling path while preserving
   membership checks, private-room confidentiality, bounded mesh selection, and
   crash expiry.
2. Replace frontend polling, per-command automatic synchronization, call
   heartbeat scheduling, and service lifecycle timers with one explicit
   app-owned scheduler and publication path.
3. Consolidate fragmented local home state into one transactional durable-state
   authority while keeping OS-keystore capabilities separate and recovery
   export derived and inspectable.
4. Collapse acceptance/sync/storage glue into one transactional
   validate-and-admit boundary while retaining a pure, independently testable
   semantic validator.
5. Generate shell dispatch, payload contracts, frontend adapters, CLI/HTTP
   adapters, and automation metadata from one typed semantic command
   definition.
6. Remove an unused sidecar or embed an actually required surface as an optional
   mode; route product CLI behavior through the application command authority;
   and keep repository provenance tooling outside the product runtime/build
   graph where practical. Preserve inhabitant-surface obligations if that
   documented external boundary is still required.
7. Replace O(history) known-event exchange with bounded DAG-frontier/want
   negotiation; transmit reusable identity/delegation proofs once by content
   address; and fetch content-addressed blobs independently from signed message
   metadata.
8. Explore one signed encrypted event representation rather than a redundant
   signed outer and signed inner event, provided excluded peers can forward it
   and admitted peers still authenticate and semantically validate it.
9. Use disposable incremental projections and replay checkpoints rather than
   replaying complete histories for every snapshot, without allowing a cache or
   index to become a second truth.

Rank these after measurement and topology mapping. A long file or high line
count alone is not a compression target. Moving code without retiring an
authority, representation, boundary, coordination edge, or physical category
of work is not compression.

### Boundaries that must not be collapsed accidentally

- principal root, device authority, recovery capability, and encryption
  capability;
- governance membership and endpoint/routing availability;
- authoritative accepted facts and disposable projections or indexes;
- protocol validation and frontend presentation;
- optional hosted availability and protocol authority;
- private-room decryption rights and ordinary ciphertext retention;
- release distribution location and update/artifact integrity;
- human/agent affordances and the shared semantic command authority beneath
  them.

These separations carry named security, trust, recovery, or authority load. They
are not duplication merely because they touch the same workflow.

### Required procedure for compression changes

Before implementation:

1. Cite the exact defended statements and source sections.
2. State accepted revisions and exclusions separately from preservation.
3. Map the current authorities, representations, boundaries, schedulers, and
   coordination edges.
4. Name the complete topology or embodiment category the candidate removes and
   the surviving authority that will own the behavior.
5. Keep physical-resource fitness separate from engineering cost and project
   admissibility.

During and after implementation:

1. Carry a representative real fact through input, authoritative decision,
   durable transition when required, propagation, projection, and observable
   output.
2. Re-run the identity recovery/revocation, invite onboarding, offline-inviter
   forwarding, public feature, private-room/recovery, workbench/palette, and
   media preservation slices affected by the change.
3. Inspect retained and transmitted artifacts when storage, encryption, sync,
   or packaging changes.
4. Exercise the packaged/native surface when lived behavior changes; use the
   field test for non-loopback reachability claims.
5. Remove the obsolete authority or representation completely. Do not leave a
   parallel implementation “for safety” without an external owner, removal
   condition, and verification path.
6. Update the contract and the more specific document when meaning, envelope,
   evidence, or an external contract changes.
7. Checkpoint coherent collapses with a provenance snapshot, commit, and push.

Use this litmus test:

> If replacing X changes who may decide a fact, who may decrypt it, whether
> recovery preserves identity, whether operation requires a privileged service,
> or whether the same user-visible workflow completes, X touches an invariant.
> If it changes only how many files, crates, processes, timers, serializations,
> copies, or bytes embody those truths, X is a compression candidate.
