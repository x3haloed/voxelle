# Voxelle Repository Threat Model

## Overview

Voxelle is a native, local-first, IPv6-only peer-to-peer collaboration system for private spaces of roughly 2–50 members. Its product surface combines durable person-level identity, independently authorized devices, signed space governance, append-only room-event DAGs, direct IPv6 QUIC synchronization, ordinary-member store-and-forward, encrypted private channels, small direct WebRTC calls, a Tauri/WebView desktop workbench, an independently testable CLI, and an optional local HTTP/SSE inhabitant service. It deliberately has no required Voxelle-operated authority for identity, membership, recovery, routing, storage, synchronization, encryption keys, or update integrity.

This model is repository-scoped. The authoritative preservation envelope is `docs/TRUTHFUL_SYSTEM_CONTRACT.md`, followed by `docs/P2P_INVITE_SPACE_CHAT_RFC.md`, `docs/IPV6_NATIVE_P2P_SPEC.md`, `docs/UI_ONTOLOGY.md`, `docs/INSTALL_UNSIGNED.md`, `docs/FIELD_TEST.md`, and `docs/inhabitant-surface-v0.md` in the precedence order stated by `AGENTS.md`. Draft protocol text does not override the truthful-system contract or verified merged behavior, notably stable principal continuity across root rotation.

The current implementation is primarily divided into:

- `crates/voxelle-core`: principal genesis and identity proofs, device authorization and revocation, signed events, deterministic governance, semantic admission, bounds, room DAGs, and call participation rules;
- `crates/voxelle-store`: SQLite retention of accepted events, monotonic identity heads, and validated local state;
- `crates/voxelle-sync`: bounded causal-difference selection and admission-before-storage;
- `crates/voxelle-net`: IPv6 QUIC, certificate pinning, signed device handshakes, diagnostics, and bounded bidirectional room sync;
- `crates/voxelle-app`: encrypted identity vaults, invites, recovery, private-room key wrapping and content encryption, peer orchestration, projections, and the semantic command host;
- `crates/voxelle-tauri-host` and `web`: the native WebView bridge, dockable UI, attachment and media surfaces;
- `crates/voxelle-cli` and `crates/voxelle-inhabitantd`: human and agent-facing control surfaces over the same application authority;
- `.github/workflows/release-artifacts.yml` and `scripts/package-*`: unsigned native artifact production and checksum generation.

The distinctive threat is adversarial convergence among open implementations. The protocol and source are public, so a bad-faith participant can replace the official client, bypass sender-side UI restrictions, choose valid-but-hostile timestamps and parent sets, create many branches, withhold selected events, equivocate between peers, retain data indefinitely, or automate traffic at machine speed. Security must therefore follow Kerckhoffs's principle: recipients must authenticate, authorize, bound, admit, retain, decrypt, and project data safely without relying on obscurity or a sender's official UI.

This model borrows the useful actor separation and candid limitation style of [Quiet's threat model](https://github.com/TryQuiet/quiet/wiki/Threat-Model): owner, member, removed member, non-member, passive dragnet, active network attacker, malware, and update provider are meaningfully different adversaries. Voxelle adds protocol-specific actors for a malicious custom client, leaked invite holder, stolen recovery-kit holder, ordinary forwarding peer, local automation caller, and compromised build or mirror. Quiet's lessons about member spam/withholding, impossible remote deletion, metadata leakage, archived ciphertext plus later compromise, and update-provider power apply directly, but Voxelle's direct IPv6, recoverable principals, signed governance DAG, bearer recovery capability, and WebView/agent bridges create additional boundaries.

The threat model is not a claim that every desired property has external or production evidence. The contract's evidence horizon still applies: Windows first launch, non-loopback networks, diverse firewalls, physical media devices, accessibility combinations, and unsigned-install policy variants require external testing. This document describes what reviews must protect and how to calibrate failures; it is not a vulnerability report for a particular diff.

## Threat Model, Trust Boundaries, and Assumptions

### Security assets and privileges

The highest-value assets are:

1. **Principal continuity.** The immutable principal ID, signed identity genesis, ordered identity proof, current root authority, and the fact that recovery rotates authority without changing the principal.
2. **Capability separation.** Principal-root, recovery, device-signing, member-encryption, and QUIC transport capabilities must not become one interchangeable secret or one implicit authorization decision.
3. **Recovery authority.** A `.voxrecover` kit contains a bearer recovery secret and authenticated capsule. Possession is intentionally sufficient to rotate the root, revoke old devices, restore current private-room keys, and continue as the principal.
4. **Space governance.** The accepted space genesis and deterministic governance log decide membership, bans, roles, permissions, invites, room definitions, private membership, device revocation, and key epochs.
5. **Admission integrity.** Only events passing the single Rust acceptance path may become durable accepted facts. Transport success, signature validity, successful decryption, fixture state, or UI display is not admission.
6. **Durable convergence.** Accepted facts, monotonic identity heads, and reconstructible projections must survive restart, retry, reordering, missing history, forwarding, and ordinary partitions without creating a second truth.
7. **Private-room confidentiality.** Excluded peers must not obtain room epoch keys or plaintext. Admitted peers must still validate the decrypted inner semantic event.
8. **Local secrets and state.** The OS-held identity-vault unlock key, encrypted `identity.json`, SQLite state, encrypted room-key rows, QUIC private credential, recovery exports, clipboard contents, and media devices are sensitive at different levels.
9. **User-visible truth.** Authorship, moderation, membership, room visibility, failures, unread state, notifications, call participation, and degraded topology must reflect admitted Rust-owned meaning. A WebView, preview fixture, or agent adapter must not silently invent protocol truth.
10. **Availability without authority.** Ordinary peers and optional providers may improve reachability, retention, and recovery availability, but may not gain the ability to decide valid identity, membership, content, or keys.
11. **Release integrity.** Users must be able to distinguish project-produced bytes from corruption or substitution without being told that unsigned artifacts have vendor-backed authenticity they do not possess.

### Adversary classes

**Space authority or administrator.** Controls the space-authority principal or has governance roles. This actor may legitimately issue and revoke invites, create channels, grant roles, ban members, and moderate content within granted policy. Abuse of an explicitly granted power is generally governance policy, not a software vulnerability. It becomes a security failure if the actor can impersonate another principal, rewrite signed history, decrypt a private room from which it is excluded, acquire root/recovery secrets, bypass deterministic rules, or cause peers to derive irreconcilable governance from the same accepted facts.

**Invited member using a malicious client.** Has a legitimate principal, device authorization, membership, and possibly room or governance permissions. The client may ignore all sender-side controls. It can sign arbitrary candidate events as itself; choose parents, timestamps, labels, profiles, attachment metadata, SDP/ICE, and delivery subsets; generate concurrent branches; spam within receiver-accepted bounds; selectively relay; retain plaintext and ciphertext; and present different valid data to different peers. It cannot be assumed to honor deletion, rate limits, UI warnings, key erasure, heartbeat cadence, or protocol etiquette.

**Invited automation principal.** Has member or scoped agent capabilities but can operate faster and more persistently than a human. Machine-scale fan-out, event generation, sync churn, invite redemption, attachment generation, and call signaling make per-principal and aggregate bounds important even in invite-only spaces.

**Banned member, removed private-room member, or revoked device.** Retains all material learned before removal: public history, old private plaintext, old epoch keys, peer addresses, and protocol knowledge. It can continue sending network traffic and replaying old data. It may exploit stale governance at partitioned peers, author-chosen timestamps, unrotated key epochs, or stale endpoint records. Revocation is not retroactive erasure.

**Leaked-invite holder or uninvited non-member.** Has no legitimate membership unless it possesses a still-valid bearer invite. A leaked unbound invite can be redeemed by multiple fresh principals until expiry or observed revocation; strict single-use counting is explicitly not claimed. Without an invite, the actor can still connect to public IPv6 listeners, send malformed handshakes and frames, probe reachability, exhaust connection resources, and attempt to impersonate member identifiers.

**Ordinary retaining or forwarding peer.** Is an authorized room member and may be always online. It can observe accessible content and metadata, retain it forever, delay or omit selected events, provide stale subsets, lie about availability, and attempt an eclipse. It has no additional protocol authority merely because other members rely on it for uptime.

**Passive dragnet.** Observes and archives network metadata and encrypted traffic at scale. Direct IPv6 reveals endpoints and communication timing; anonymity and metadata privacy are explicitly out of scope. The dragnet may later combine archives with a compromised member device or recovery secret. QUIC transport encryption does not erase membership metadata known by peers or room ciphertext stored on their devices.

**Active network attacker.** Can observe, delay, drop, replay, reorder, redirect, partition, or block traffic; advertise hostile network paths; and degrade IPv6. It cannot break standard cryptography. Denial of all network connectivity is possible and out of scope, but silent acceptance of forged facts, private-content disclosure, or persistent corruption from network manipulation is in scope.

**Endpoint or bootstrap manipulator.** Controls an address, imported peer record, stale endpoint, compromised invite-sharing channel, or replacement peer. It attempts endpoint poisoning, certificate substitution, peer-ID confusion, self-asserted identity, topology eclipse, or connection to unintended local/link-local/global addresses. Endpoint knowledge is availability data, never membership authority.

**Local malware or same-user process.** Can act with some or all privileges of the logged-in user, call loopback services, access the WebView, read clipboard or files, request media, or use OS credential APIs. Full same-user compromise may be able to act as the victim and is not completely preventable. The design should still prevent one non-secret file disclosure from yielding all capabilities, keep recovery exports exceptional, and limit remote escalation through local bridges.

**Stolen recovery-kit holder.** Possesses the recovery card and capsule. This is a full principal-takeover capability by design, not merely a password reset. The holder can rotate the root, revoke legitimate devices, restore private-room keys in the capsule, and sign future facts. Detection, safe storage guidance, guardian policy, and any future recovery-key rotation are therefore high-value controls.

**Malicious attachment or media sender.** Is usually an invited member. It supplies file bytes, filename, MIME type, profile text, message text, SDP, ICE candidates, and media streams intended to cross Rust, WebView, OS file-handler, and WebRTC boundaries. It seeks code execution, privileged WebView navigation, parser exploitation, local-network access, device capture abuse, or misleading presentation.

**Malicious local automation caller.** Can reach `voxelle-inhabitantd` or invoke the Tauri bridge. Depending on exposure, it can read snapshots and exercise the full semantic command set, including initialization, joining, messaging, governance, networking, and preference mutation. Loopback binding limits remote reachability but is not caller authentication; explicitly binding the sidecar beyond loopback materially changes the threat model.

**Contributor, dependency, CI, release-host, or update-provider attacker.** Can introduce source or dependency backdoors, change workflows, produce malicious native artifacts, replace an artifact and checksum together, or serve a modified download. Open source improves auditability but does not make produced binaries trustworthy. A compromised distributor that controls both artifact and adjacent `SHA256SUMS.txt` is equivalent to malware distribution.

### Trust boundaries

| Boundary | Less-trusted side | More-authoritative side | Required property |
| --- | --- | --- | --- |
| Invite import | JSON/file/clipboard and out-of-band sender | Signed space genesis, invite event, and admission state | Parse and bound first; signature, authority, expiry, space binding, and revocation decide admission. Bootstrap endpoints never grant membership. |
| Endpoint/peer import | Address, certificate bytes, labels, asserted peer/device IDs | Connectivity selection | Treat as replaceable hints; validate structure and certificate binding; bind the live device and principal before authorizing room data. |
| QUIC connection | IPv6 network and remote implementation | Authenticated transport session | Bound frame/stream resources; verify certificate and possession; bind device to principal authorization; never use address or an asserted identifier as room authority. |
| Sync | Remote heads, event batches, ACK counts, ordering, omissions | `accept_event` plus `Store::insert_accepted_event` | Authorize the room before transfer, bound heads/events/bytes, validate each fact before storage/forwarding, and keep idempotence. |
| Identity proof | Remote genesis, changes, delegation, claimed current head | Derived principal/device state and monotonic SQLite head | Verify signatures, sequence, previous link, scopes, expiry semantics, and extension of the locally known proof; reject rollback and forks. |
| Governance DAG | Signed but potentially hostile authorized events and author clocks | Deterministic membership/role/room/key state | Same accepted event set must yield the same state. Missing ancestors, concurrent events, timestamps, and tie-breaks must not grant attacker-selected authority. |
| Private ciphertext | Stored/forwarded envelope and key packages | Decrypted semantic event | Gate key delivery by admitted membership and epoch; authenticate ciphertext; revalidate inner author, permission, references, and bounds after decryption. |
| Recovery import | Bearer `.voxrecover` file and retained peer history | Fresh principal vault and new device authority | Authenticate/bind capsule, require a fresh home, extend the latest identity proof, revoke old devices, resync, and propagate the new head. |
| Local persistence | Filesystem, SQLite rows, crash/restart, backup tools | Reconstructed accepted truth | Separate capability files, protect secrets, make insertion idempotent/crash-safe, reject stale identity proofs, and never treat a disposable projection as authority. |
| WebView/UI | Untrusted replicated text/files/media and DOM state | Serialized Rust command host | Render as data, not markup; prevent privileged navigation/script injection; validate typed command payloads; UI cannot reconstruct authorization. |
| Inhabitant HTTP/SSE | Local or operator-exposed HTTP client | `ShellState` and all semantic commands | Default loopback, explicit exposure warning, caller authentication if non-loopback, request bounds, serialized execution, and visible attribution. |
| Live media | Signed signaling plus remote WebRTC endpoint | Camera/microphone and direct media session | Only admitted selected participants signal; bind target and room/call; stop capture on leave/failure; do not convert a relay into media authority. |
| Build/release | Contributors, dependencies, CI runners, artifact host, mirrors | User-installed executable | Reproducible/reviewable inputs, least-privileged CI, independent authenticity where claimed, checksum verification, and honest unsigned-install guidance. |

### Security invariants

Reviews should treat the following as repository-wide invariants:

- A principal is not a device, root key, endpoint, hostname, QUIC certificate, OS credential entry, or service account. Root rotation must preserve the principal and invalidate lost-device authority.
- Recovery, root, device, member-encryption, and transport keys may be related by an explicit protocol but may not be accepted interchangeably.
- No address, certificate, relay, retaining peer, bootstrap record, optional provider, WebView, automation adapter, or release host decides protocol identity or membership.
- Every durable event follows one path: parse and bound, authenticate, evaluate the applicable identity and governance state, validate semantics, admit, store, sync, project. Retry and forwarding do not create alternate acceptance.
- Governance is deterministic from accepted signed facts. The same complete fact set cannot produce different membership, ban, role, room, invite, private-membership, revocation, or key-epoch decisions.
- A newer accepted identity head cannot be rolled back or forked by later delivery of an older otherwise valid event. Recovery revocation must defeat replay by lost devices.
- Unknown, missing, duplicated, reordered, or concurrently authored events cannot cause unauthorized state or permanent non-convergence. Bounds must fail safely rather than create an attacker-controlled durable wedge.
- Private-room access follows admitted membership and explicit epochs. Decryption proves knowledge of a key, not authorization or semantic validity.
- Public-room content is authenticated but not end-to-end encrypted. Public-space members and retaining peers can read it; the UI must not suggest otherwise.
- A member who has legitimately received plaintext or an epoch key can retain it. Redaction is a signed projection/tombstone, not remote erasure. Ban and key rotation protect future authority/content only within explicitly defined semantics.
- The frontend, CLI, and inhabitant service are affordances over the same Rust commands. They may not mutate SQLite directly or implement competing authorization rules.
- Live media remains direct among two to four admitted selected participants. Signaling, crashes, missing devices, slot release, and degraded states remain explicit.
- Optional providers and ordinary peers may cause temporary or permanent availability loss by withholding data, but cannot forge accepted facts. No universal delivery or anonymity is claimed.
- Release instructions must distinguish integrity from authenticity. A hash is useful only under an authentic manifest; bypassing Gatekeeper or SmartScreen must remain narrow and per-app.

### Assumptions and explicit non-goals

- Cryptographic primitives and their libraries—Ed25519, SHA-256, X25519, XChaCha20-Poly1305, TLS 1.3/QUIC, OS CSPRNGs—are assumed secure when correctly used. Side-channel and cryptanalytic review is separate.
- The official macOS or Windows installation, OS credential store, WebView, SQLite library, and Rust runtime are initially non-malicious. A fully compromised OS can generally act as the user.
- Users exchange initial invites through an authentic enough out-of-band channel for their risk. Confidentiality of the invite channel is desirable because an unbound invite is a bearer admission capability.
- Each security decision is made from locally retained signed facts; temporary divergence during partitions is expected. Eventual convergence assumes eventual honest network paths and that at least one ordinary peer retains each needed fact.
- Author timestamps are attacker-controlled claims constrained by signature/delegation and a future-skew rule; they are not trusted wall-clock attestations. Invite expiry and creation-time authorization are not partition-proof leases.
- Direct IPv6 intentionally reveals IP addresses and coarse online/communication timing to connected peers and network observers. Voxelle does not promise anonymity, Tor-level unlinkability, membership hiding from authorized peers, universal reachability, or resistance to complete network blocking.
- Public rooms are not E2EE. Private-room E2EE does not imply that recipients cannot copy content. The current static member encryption capability and retained epoch packages should not be assumed to provide Signal-style forward secrecy or post-compromise security unless a separate reviewed protocol establishes it.
- Strict single-use invite counting, centralized admission arbitration, guaranteed push notifications, delivery while every member is offline, large public communities, large calls, browser protocol participation, mobile clients, and remote erasure are out of scope.
- The repository is not deployed and existing development homes are disposable. Compatibility and migration paths are not part of the current attack surface unless that status changes.
- Developer-only preview fixtures and tests are not product protocol authorities. They matter to supply-chain integrity but cannot establish runtime security claims.

## Attack Surface, Mitigations, and Attacker Stories

### 1. Principal identity, device delegation, and recovery

**Attacker-controlled inputs.** Identity genesis and proof chains embedded in delegations; identity changes; author/device public keys; scopes; validity times; recovery kits; old events carrying stale proofs; concurrent or forked identity histories.

**Primary attacker stories.** A lost device continues signing; an attacker replays an old but once-valid delegation after recovery; a malicious peer presents a shorter or same-sequence fork; a stolen recovery kit rotates the root and locks out the owner; malware steals only one key and attempts to use it as another capability; peers observe different identity heads and accept different devices.

**Existing controls.** `voxelle-core` derives the stable principal from self-signed genesis, restricts the recovery key to root rotation, orders identity changes by sequence and previous hash, scopes device authorization, and verifies event/delegation signatures. `voxelle-store` retains a monotonic identity head and rejects stale or forked proofs. Recovery creates a new root and device and revokes old devices. `voxelle-app` encrypts root, device, and recovery secrets with XChaCha20-Poly1305 under an OS credential-store key in release builds, writes recovery exports with restricted Unix permissions, authenticates the capsule, and requires a fresh home.

**Review focus.** No code path may validate a device signature while trusting a separately self-asserted principal ID. Identity proof comparison must remain prefix/extension-safe across stores and all rooms. Recovery is a full takeover boundary and must never be logged, placed in ordinary clipboard history without warning, stored in SQLite plaintext, or accepted without matching genesis/latest proof. The stable recovery secret also derives the member encryption key, so recovery-secret compromise has confidentiality consequences beyond identity takeover. Future guardian recovery must not introduce a privileged service or collapse threshold shares into a routinely available bearer secret.

### 2. Governance, permissions, clocks, and adversarial DAGs

**Attacker-controlled inputs.** Valid signed governance events, event parents, event IDs through content, timestamps, concurrent branches, role definitions, invite actions, channel changes, bans, private member sets, key epochs, and selective delivery.

**Primary attacker stories.** A member with limited governance rights constructs an event sequence that gains wider rights; two authorized admins create conflicting operations during a partition; a participant manipulates timestamps/tie-breakers to win deterministic ordering; a banned member backdates a post to pre-ban state; a member references missing or hostile parents; an attacker creates more DAG heads than sync accepts; a retaining peer shows different valid subsets to different members.

**Existing controls.** Events use canonicalized signed fields and content-derived IDs; parents are canonicalized; future timestamps are bounded by five minutes; governance is projected through a deterministic topological order; permissions are checked in `accept_event`; unknown event kinds default to the least-privileged post scope; member, role, channel, private-key-package, message, attachment, and call bodies have semantic bounds. Accepted facts are immutable; edits and redactions are new events.

**Review focus.** Determinism must be tested with permutations, missing ancestors, concurrent conflicts, and malicious clocks—not only serialized happy paths. Authorization-at-event-time must have an explicit rule for ban, invite revocation, device revocation, and root recovery; signed author time cannot by itself prove when an event was created. Branch/head limits must not let one authorized member permanently halt room sync. Governance-body validation and governance-state application must implement the same rules; a fact accepted by one but ignored or interpreted differently by the other is dangerous. Authority-only actions and role-delegable actions must remain intentional and documented.

### 3. Invites, onboarding, bootstrap records, and topology eclipse

**Attacker-controlled inputs.** Invite JSON, clipboard/file contents, expiry, bootstrap arrays, labels, addresses, certificate DER, fingerprints, asserted peer/device/authority IDs, imported standalone peer records, stale records, and the out-of-band delivery channel.

**Primary attacker stories.** An invite is modified or belongs to another space; an attacker leaks a valid unbound invite and creates many principals; a revoked invite is redeemed against a stale partition; bootstrap peers all point to the attacker or dead addresses; a peer record redirects a user to a hostile certificate/device; a valid endpoint is mistaken for membership; an inviter goes offline and a forwarding peer gives the joiner a censored governance subset.

**Existing controls.** Space genesis, invite, and invite-revocation events are signed by the space authority, bounded to one to eight bootstrap peers, expire where applicable, and are admitted as governance facts. `SpaceInviteFileV1` validates space, invite, and bootstrap-space consistency. QUIC endpoints include pinned certificate material and device IDs. Before creating local identity state, joining uses an ephemeral governance-only preflight against each reachable bootstrap peer and refuses an invite whose admitted revocation is learned; successful joining then pushes a signed member event and pulls history through ordinary peers. Endpoint JSON alone is not meant to grant membership. Fresh onboarding displays bounded untrusted claims for the target space, authority, expiry, and bootstrap count before submission and explicitly warns that an unbound bearer invite can be reused; the People surface exposes bounded expiry choices, Rust-projected active invitations, and explicit revocation confirmation with the stale-partition limitation. Rust remains the admission authority.

**Review focus.** Invite possession is authority to attempt membership, so import UX must display the target space/authority and the limitations of expiry/use count. Revocation must be evaluated against sufficiently complete governance and must fail visibly when history is unavailable. Standalone peer records are untrusted availability inputs even when structurally valid. Live transport identity must be cryptographically bound to the authorized principal before governance or private-room content is released. Automatic peer selection should use plural paths where available and report when all paths share one bootstrap/eclipsing source.

### 4. IPv6 QUIC handshake, diagnostics, and synchronization

**Attacker-controlled inputs.** Connection rate, QUIC handshakes, claimed roles and IDs, certificate material, stream count, frames, room IDs, up to bounded head sets and event batches, ACK counts, truncation flags, timeouts, connection churn, and network ordering.

**Primary attacker stories.** A non-member floods the public IPv6 listener; a malicious device proves possession of its own key while claiming another principal; a client requests a private room before membership is established; a peer sends oversized JSON or many streams; an attacker advertises 256+ heads, repeatedly sends rejected events, lies in ACKs, stalls the reciprocal push, or uses diagnostic endpoints as resource amplifiers; an active network attacker blocks selected governance events to preserve stale authorization.

**Existing controls.** QUIC encrypts transport and pins the presented server certificate; handshakes prove possession of a device key and bind a certificate fingerprint; outgoing connections check expected device and certificate; frames are capped (16 KiB handshake, 512 KiB sync), event batches at 4096, and heads at 256; connects and reciprocal sync have timeouts; private-room and membership authorization occurs before sync transfer; every received event passes semantic acceptance and idempotent storage.

**Review focus.** A signed device handshake is not sufficient unless the device is bound to the claimed principal's current identity proof. Inbound and outbound checks must be symmetric. The spec requires connection/rate/queue bounds; reviews should verify actual runtime-wide concurrency, per-address and per-principal limits, backpressure, and repeated-failure handling rather than infer them from per-frame caps. Unauthorized requests should reveal minimal membership information. ACKs are telemetry, not truth. A hostile peer must not cause permanent convergence failure by exploiting truncation, missing parents, rejected prefixes, or excessive heads.

### 5. Durable store, replay, and projection reconstruction

**Attacker-controlled inputs.** Valid and invalid event JSON, duplicates, arrival order, SQLite file corruption or rollback, local state values, disk exhaustion, crash timing, and restored backups.

**Primary attacker stories.** An attacker gets unvalidated events durably inserted; a stale backup rolls back an identity head; duplicate replay creates repeated UI messages or side effects; corrupt local preference/config rows become protocol authority; full-retention spam fills disk; a crafted graph causes expensive repeated full-room reconstruction; a local attacker edits SQLite to grant membership or keys.

**Existing controls.** The `AcceptedEvent` wrapper makes validation a type-level prerequisite to insertion. Event IDs are primary keys and insertion is idempotent. SQLite WAL and busy timeout support crash behavior. Identity heads advance only on strictly newer extending proofs. Space meaning is reconstructed from an admitted signed genesis, while local rows select or project it. The contract treats local UI/read/search state as non-protocol authority.

**Review focus.** All database openings, recovery imports, tests, CLI tools, and future maintenance jobs must preserve the acceptance gate. Local tampering is detectable only if signed facts are revalidated; local device preferences are allowed to be mutable but cannot grant remote authority. Full retention plus invited-member spam is a serious availability risk: event count, graph width, disk budget, compaction, and recovery from low disk must be explicit before long-running use. Search indexes, unread cursors, and view layouts must remain disposable projections.

### 6. Private-room encryption, key epochs, and later compromise

**Attacker-controlled inputs.** Member encryption keys published at join, private member lists, key packages, epochs, ephemeral public keys, nonces, ciphertext, encrypted inner kinds/bodies, archived ciphertext, and membership/key-rotation timing.

**Primary attacker stories.** A channel creator omits or substitutes a recipient package; an excluded peer receives room events or a key package; a removed member continues reading because no new epoch is created; a malicious recipient republishes plaintext or keys; an attacker swaps inner semantics under an otherwise valid envelope; later theft of the stable recovery/member-encryption secret decrypts archived room-key packages; recovery restores a stale key set; an old epoch is replayed as current.

**Existing controls.** Private channel creation requires current members with published 32-byte X25519 keys, packages exactly one random room key to each member, and signs the governance event. Per-epoch content uses XChaCha20-Poly1305 with room/epoch/author associated data. Sync checks both peers' private membership before transfer. The application decrypts and then reruns ordinary semantic validation on the reconstructed inner event. Room-key state is encrypted at rest under the identity-vault key and included in the authenticated recovery capsule. The human rotation surface projects the admitted epoch and private-member count, requires explicit confirmation, and states that rotation protects future content without erasing material recipients already retained; Rust remains the key-generation, packaging, admission, storage, and synchronization authority.

**Review focus.** Key distribution must follow admitted governance, not merely a supplied member list. Ban/removal semantics must state when a new epoch is mandatory; old recipients cannot be made to forget old epochs. Recovery and static member encryption intentionally favor continuity, but they should not be described as forward secrecy or post-compromise security. Epoch monotonicity, package uniqueness, recipient binding, ciphertext AAD, nonce randomness, and refusal to project undecryptable/invalid inner facts require negative tests. Ordinary ciphertext retention must not imply decryption rights.

### 7. Malicious members, moderation, redaction, and human identity

**Attacker-controlled inputs.** Display names, profiles, messages, mentions, reactions, threads, edits, tombstones, roles, filenames, Unicode, and concurrent moderation.

**Primary attacker stories.** A member selects a confusable display name, creates mention spam, edits content after it is quoted, disputes transcript ordering, sends different branches to different peers, or refuses to relay. A moderator abuses an authorized ban/redaction. A removed member preserves screenshots/database copies. A malicious client renders a locally falsified transcript even though stored facts are valid.

**Existing controls.** Cryptographic principal IDs and device signatures—not display names—establish authorship. Message/profile lengths, targets, role permissions, attachment hashes, mentions, and moderation rights are validated. Edits and redactions are signed immutable events. Deterministic DAG ordering stabilizes honest projection. Rust emits the durable ViewModel consumed by the UI.

**Review focus.** UI labels must never be the sole trust indicator; collisions and confusables need principal/device context for sensitive actions. Redaction means an authorized tombstone, not deletion from hostile recipients or backups. A malicious member's withholding and spam are realistic threats, while their refusal to delete legitimately received content is an explicit limitation. Local UI code must not hide, duplicate, reorder, or relabel accepted events in a way that changes security meaning.

### 8. Attachments, search, WebView, and native bridge

**Attacker-controlled inputs.** Up to 256 KiB attachment bytes, filename, MIME, base64, hashes, all replicated text, `data:` URLs, file-picker results, DOM events, clipboard contents, WebRTC signaling, and any future link/embed content.

**Primary attacker stories.** A crafted attachment navigates the privileged WebView or invokes a native handler; a filename/MIME pair misleads the user; replicated HTML/script becomes XSS; a malicious link gains access to Tauri globals; an injected script invokes `execute_shell_command` to export capabilities, join a space, ban a member, or start networking; search or rendering creates CPU/memory denial; preview mode accidentally mutates fixture truth and is mistaken for the native app.

**Existing controls.** The UI generally constructs DOM nodes and assigns untrusted values through `textContent`; Rust validates replicated bodies and attachment hashes/sizes. Tauri exposes one serialized command entrypoint backed by a mutex and typed Serde requests, and its declared capability file only enables native snapshot event listening. Runtime truth stays in Rust. The human surface requires explicit modal review before product-package installation, staged-generation activation, rollback, or release-root transition, including when the semantic command starts in the palette; this is user-presence friction, not update authentication. Bounded previews label portable update and trust-transition metadata as untrusted while the native kernel remains the only signature, role, sequence, downgrade, compatibility, and trust-set authority.

**Review focus.** The WebView is a high-privilege origin because a script that reaches the bridge can exercise the semantic command host. CSP, navigation policy, remote-content exclusion, `data:` attachment behavior, MIME handling, external opener behavior, and all DOM injection sinks deserve high scrutiny. Type-safe command parsing does not authenticate who called the bridge. Sensitive commands may need user-presence or explicit confirmation even when syntactically valid. Generated shell contracts must match Rust; fixture/preview clients cannot be security evidence.

### 9. Inhabitant HTTP/SSE and CLI automation

**Attacker-controlled inputs.** `--host`, `--port`, home paths, discovery-file path, HTTP command IDs and JSON bodies, connection count, SSE clients, and any process able to read the discovery file or connect to the selected address.

**Primary attacker stories.** A same-user process discovers the ephemeral port and executes privileged commands; an operator binds the unauthenticated service to LAN/global IPv4/IPv6; a malicious webpage reaches loopback through browser behavior; many SSE clients or slow HTTP requests consume resources; two automation clients race governance or UI actions; an agent acts on stale snapshot state without human visibility.

**Existing controls.** The default bind is `127.0.0.1`; commands are serialized through the same `ShellState` mutex and typed command dispatch as the desktop; unknown commands fail closed; activity and returned snapshots expose effects. The sidecar is optional and separate from ordinary one-process desktop use.

**Review focus.** Loopback is a reachability restriction, not an authorization credential. Non-loopback binding should require an explicit security mode with authentication, origin protections, TLS as appropriate, and clear warnings; otherwise it should be refused. Discovery files should not contain secrets and should have safe permissions. Request/body/concurrency limits and cancellation must be explicit. Snapshot-before-action is a consistency aid, not authorization; high-impact commands need attribution and may need optimistic state/version checks.

### 10. Direct WebRTC media and signaling

**Attacker-controlled inputs.** Call joins, heartbeats, leave events, target peer IDs, SDP offers/answers, ICE candidates, timing, remote media tracks, connection-state transitions, and camera/microphone permission prompts.

**Primary attacker stories.** A member floods retained signaling, targets a nonparticipant, occupies one of four deterministic slots, replays heartbeats, injects hostile SDP/ICE, causes unwanted capture, keeps tracks alive after leave, correlates IPs, or induces the UI to say video is active when only audio/no capture exists. An optional relay becomes a media eavesdropper or room authority.

**Existing controls.** Call events use ordinary signed admission, membership, room visibility, and private encryption; signaling fields and participants are bounded; targets must be active selected participants; the active set deterministically takes four and expires after 90 seconds; media is direct WebRTC; the UI requests capture and truthfully falls back to audio for known camera-unavailable cases.

**Review focus.** Signaling validity is not proof that SDP/ICE is harmless to the WebRTC stack. Ensure capture occurs only after an explicit user action, tracks stop on leave/crash/navigation, remote media is associated with the authenticated participant, and permission-denial/degraded states are accurate. No STUN/TURN/SFU is currently used; adding one changes metadata and trust boundaries and must not grant protocol or media authority. IP exposure among call participants is expected.

### 11. Availability, abuse, and open-protocol economics

**Attacker-controlled inputs.** Connection/event frequency, number of principals admitted by a leaked invite, DAG breadth, attachment and ciphertext volume, peer count, endpoint churn, retries, intentionally slow streams, and selective forwarding.

**Primary attacker stories.** One invited member fills full-retention stores; an agent generates maximum-size valid events continuously; an attacker creates many concurrent heads until sync refuses the room; a forwarding peer advertises availability then blackholes events; all bootstrap hints depend on one operator; repeated automatic sync amplifies work; a malicious peer makes expensive signature/governance/decryption checks while contributing no accepted facts.

**Existing controls.** Small private group scope, invite gating, message/body/attachment/call field limits, bounded head and event batches, connection timeouts, idempotent IDs, and plural known peers reduce exposure. Network failures and stalled/fragile health are intended to be visible.

**Review focus.** Invite-only is not abuse-proof: insiders and leaked invites are first-class adversaries. Verify per-address, per-device, per-principal, per-room, and global budgets; connection and stream concurrency; validation CPU; disk quotas; branch width; retry jitter; and failure memory. Bounds must be applied before allocation/deserialization where possible. Availability mechanisms must remain replaceable and plural; reliance on one always-on peer is an operational single point even if it has no cryptographic authority.

### 12. Build, dependency, packaging, and unsigned distribution

**Attacker-controlled inputs.** Pull requests, Rust/npm/Tauri dependencies, lockfiles, GitHub Actions, action tags, toolchain resolution, CI runners, release tags, uploaded artifacts, checksum manifests, mirrors, and user instructions to bypass OS warnings.

**Primary attacker stories.** A dependency or contributor exfiltrates keys or alters validation; a mutable CI action/tag is replaced; the release host serves a malicious binary and matching checksum; a lookalike mirror publishes its own manifest; an attacker persuades users to disable Gatekeeper/SmartScreen globally; local builds use unexpected toolchains or debug vault settings.

**Existing controls.** Cargo locking, repository tests, explicit local macOS and Windows packaging commands, ad-hoc macOS signing, native Tauri packaging, SHA-256 corruption manifests, an Ed25519-signed release manifest rooted in the installed kernel, an ordered domain-separated signed release-root transition chain, a capability-separated recovery root that cannot sign manifests or product packages, and narrow per-app trust instructions provide reviewability and independent package authenticity. Product generations, GitHub metadata, adjacent checksums, and unsigned local files cannot add or retire release roots. Live transitions cannot add or retire the embedded recovery root. GitHub Actions release automation is intentionally absent for the beta path. The guide explicitly rejects global Gatekeeper disablement and does not claim vendor signing.

**Review focus.** The private release key is a malware-distribution capability and must remain outside the repository, product runtime, logs, and artifacts with an offline recovery copy. Signed manifests authenticate bytes but do not prove the binary was built from reviewed source; reproducible-build and independent-builder evidence remain separate. A holder of the only currently trusted signing secret can authorize a malicious transition, so a separately protected successor or recovery key must exist before compromise. Trust-root transition rollback after local state loss, monotonic update sequence, corrupt active-package recovery, manual release-host credentials, dependency audit, generated-file verification, and exclusion of debug/test secret backends remain review targets. An update-provider compromise without a trusted release key may deny availability but must not authorize a generation.

### Repository-context vulnerability classes

The most relevant classes are:

- authorization bypass or identity-confusion across principal, device, endpoint, certificate, room, and role boundaries;
- signature/canonicalization mismatches, proof rollback/fork acceptance, and divergent governance evaluation;
- private-room key misdistribution, epoch/revocation mistakes, unauthenticated decryption, or plaintext persistence/logging;
- invite validation/revocation errors and bootstrap-to-membership authority confusion;
- resource exhaustion through frames, streams, DAG heads, full retention, attachments, sync retries, signaling, and HTTP/SSE;
- WebView XSS/navigation or malicious attachment handling that reaches the native command bridge;
- unauthenticated local/remote automation access and confused-deputy command execution;
- supply-chain and unsigned-release substitution;
- path/permission/symlink issues involving home roots, identity vaults, recovery exports, discovery files, and package scripts;
- misleading security UI that labels an unverified display name, endpoint, provider, fixture, or degraded state as authoritative.

Traditional web classes such as server-side SQL injection, multi-tenant HTTP session fixation, CSRF against a hosted account service, or cloud IAM privilege escalation are not central because Voxelle has no hosted web application or central account backend. They become relevant where analogous inputs exist: SQLite query construction, loopback HTTP commands, WebView navigation, optional providers, or release infrastructure. Remote SSRF is currently limited because peers connect to imported IPv6 endpoints by design; address-scope confusion and unwanted access to local/link-local services are the repository-specific equivalent.

## Severity Calibration (Critical, High, Medium, Low)

Severity combines impact, reachability, attacker prerequisites, affected scope, persistence, recovery cost, and whether the behavior violates an explicit contract invariant or only an excluded claim. The small invite-only envelope reduces exposure but does not automatically reduce impact: a single malicious member is a realistic adversary, and a leaked invite or public IPv6 listener can remove the trusted-member prerequisite.

### Critical

Use **Critical** for remotely or supply-chain reachable compromise of foundational authority or confidentiality across principals/spaces, especially with little user interaction or no viable recovery.

Examples:

- a network peer can forge or assume another principal, bind an attacker device to a member without an authorized identity proof, or bypass the acceptance path to store authoritative governance;
- an excluded peer or non-member can obtain private-room keys/plaintext for arbitrary rooms, or a ciphertext/canonicalization flaw permits undetectable semantic substitution;
- a malicious update, dependency, WebView injection, or remotely exposed inhabitant service yields root/recovery secrets or full native command execution for many users;
- recovery can be forged without the bearer secret, or a stale/forked identity proof can reverse a completed recovery and restore lost-device authority;
- deterministic governance fails so the same complete accepted fact set permanently grants conflicting owners/permissions and enables takeover.

The level may drop to High if exploitation requires prior full local-user compromise or possession of a recovery kit, because those already confer substantial intended authority, unless the bug expands compromise to other principals or spaces.

### High

Use **High** for realistic compromise of one space/principal/private room, durable authorization bypass, serious persistent denial by a member or non-member, or a local bridge flaw with meaningful prerequisites.

Examples:

- a malicious member bypasses role, ban, private-membership, invite-revocation, or key-epoch checks to perform an action outside granted authority;
- endpoint/principal confusion releases private history to a key-owning non-member, even if stored event signatures remain unforgeable;
- author-controlled timestamps or DAG structure let a revoked device or banned member create accepted post-revocation authority, not merely deliver an honestly preexisting event late;
- one invited client can permanently wedge convergence, exhaust all peers' disk/memory/CPU within practical time, or prevent recovery by exploiting bounded sync logic;
- XSS, malicious attachment navigation, or an unauthenticated non-loopback inhabitant endpoint can invoke sensitive commands in the victim's home;
- recovery exports, identity-vault keys, room epoch keys, or QUIC private credentials are unintentionally written to logs, world-readable files, crash reports, or public artifacts;
- release substitution defeats the stated integrity workflow for ordinary users even without compromising source control.

### Medium

Use **Medium** for bounded compromise requiring membership, significant interaction, local access, or recoverable availability/integrity loss that does not cross root/private-content boundaries.

Examples:

- an invited member can cause temporary room unavailability, excessive but bounded resource use, notification/mention abuse, or misleading non-security UI state;
- malformed event, SDP, ICE, attachment, or command input crashes one client process but does not yield code execution or durable corruption;
- peer/bootstrap manipulation creates a recoverable eclipse or stale view while the UI clearly reports degradation and alternate peers can repair it;
- a local same-user process can alter UI preferences/read state or invoke low-impact commands through loopback, but cannot extract secrets or exercise governance;
- public-room content or expected direct-IP metadata is exposed beyond the intended member set through a mistake with limited scope, without private-room compromise;
- a checksum or packaging defect weakens corruption detection but does not let an attacker replace both artifact and trusted manifest.

### Low

Use **Low** for narrow robustness, defense-in-depth, or misleading-detail issues with limited security consequence and straightforward recovery.

Examples:

- diagnostics reveal non-sensitive software/version or reachability details already observable by a connected authorized peer;
- an invalid input produces an overly detailed local error but no secret, membership, or cross-room data;
- duplicate UI activity, harmless projection inconsistency, or a restart-only preference loss occurs while authoritative accepted facts remain correct;
- a rate limit is slightly inaccurate but all allocations and durable effects remain tightly bounded;
- a hardening improvement concerns developer-only fixtures, tests, or board tooling with no path into produced artifacts or privileged CI.

Do not report expected powers or explicit exclusions as vulnerabilities: an authorized authority banning a member; a legitimate recipient copying plaintext; an old member retaining previously received content/keys; a network observer learning direct IPv6 metadata; complete network blocking; lack of strict single-use invites; unsigned OS warnings; or lack of large-scale availability. Report instead when the implementation or UI promises more, crosses an authority boundary, silently violates the documented envelope, or turns an expected limitation into a broader attack.

Repository: target_sha256_85c405c4cda65025d8a2d6285d039a2c9fb94e967ee7e8a897bf72b6c9d11b8c
Version: efd8ac3d2254d7e495b1387813412ae4b4fe1881
