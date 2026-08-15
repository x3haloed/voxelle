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
- signed, mirrorable product generations and release manifests rooted in the
  installed native kernel, with observable live activation and rollback;
  GitHub Releases is the initial distribution location but is not update
  authority. See `LIVE_PRODUCT_UPGRADE_CONTRACT.md`.

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

The live-product update authority is the Ed25519 release-root set embedded in
the installed kernel plus the kernel's monotonic package verifier. GitHub
Releases, mirrors, ordinary peers, adjacent checksum files, and update payloads
themselves cannot select or authorize a generation. Release-root rotation and
recovery must remain explicit, signed, and independent of distribution.

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

The native kernel also remains the sole authority for update verification,
active-generation selection, rollback, protocol admission, and stable semantic
command/view identities. A replaceable product generation may change only the
surface explicitly admitted by `LIVE_PRODUCT_UPGRADE_CONTRACT.md`; it cannot
create a parallel event validator, command executor, durable store, transport,
or update selector.

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
- Publish beta artifacts manually through GitHub Releases with signed packages
  and signed release manifests; do not require formal CI/CD at this stage.
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

Transport handshakes now carry the bounded identity proof that authorizes the
claimed device. The receiver derives the principal and current device key from
that proof, while the dialer binds the result to the expected peer record and
transport certificate. Room synchronization derives authority from the active
home's accepted genesis and governance state, including device revocation and
private membership; imported topology remains reachability data. SQLite commits
an accepted identity event and its monotonic identity head in one transaction.

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

Active invitations are now reconstructed from accepted governance after
restart and exposed through the Rust snapshot. Revocation uses the same signed
governance admission and ordinary-peer synchronization path as other accepted
facts. A focused two-home regression proves that a reachable ordinary peer can
convey an admitted revocation during an ephemeral governance-only preflight,
causing a stale bearer join to fail before any local identity or selected-space
state is created. This does not make revocation instantaneous across a stale
partition or provide strict single-use admission.

An exported invite is a self-contained causal entry point: its governance
event descends directly from the signed space genesis carried in the invite.
It must not claim an unexported local governance head as a parent, because that
would let head-based anti-entropy mistake missing membership history for known
ancestry. A fresh-home regression now requires the joined projection to contain
both the authority member and the new member.

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
the external Windows runner named in the evidence horizon. Replacing an
ad-hoc-signed macOS build changes its CDHash and can trigger a Keychain access
decision for the existing encrypted identity-vault key. The loading surface
explains that pending operating-system request, and the install guide limits
approval to a replacement whose signed release manifest was independently
verified. Ordinary restart evidence continues to use the exact same artifact;
replacement continuity is a separate lived gate.

The human update surface now has a direct **More → Product updates** route in
addition to its stable dockable view. Package installation, staged activation,
rollback, and release-trust rotation enter one modal review path from either
the view or command palette; missing package or trust-transition input instead
opens and focuses the required field. A rendered interaction probe exercised
modal entry, bounded authority copy, forward and reverse Tab containment,
Escape cancellation with focus return, and palette-to-input routing.
Deterministic tests enumerate the four consequential commands and distinguish
non-mutating discovery, staging, and staged-package discard. This is human
presentation evidence, not new kernel-verification, native activation, or
release-root custody evidence.

That surface now makes signed `.voxupdate` and `.voxtrust` files the ordinary
portable handoff and keeps complete JSON paste behind explicit disclosure.
Bounded untrusted previews name the package release, sequence, channel, minimum
kernel, and signer or the transition sequence, signer, and added/retired key
counts before confirmation. A rendered probe carried both file types through
their hidden file inputs into the corresponding review, kept manual text
keyboard-reachable, enabled the semantic action only after input, and preserved
the existing modal handoff. It also exposed and closed a shared menu-state gap:
More now collapses after any of its actions opens another surface. This remains
presentation evidence; frontend parsing does not verify or authorize updates.

The workbench risk has crossed its first lived gate. Every Rust-registered view
can move among all five docks by drag/drop or an accessible selector, reorder,
hide, restore, and reset. Rust rejects incomplete, duplicate, unknown, or
non-contiguous layouts and persists accepted placements in the same preference
state carried by recovery. The command palette, buttons, shortcut matching, and
shell command discovery consume the Rust-owned command registry. A packaged
Tauri run moved and hid views, filtered and invoked the palette from the native
keyboard shortcut, quit, relaunched, and projected the saved layout through the
real bridge.

The human palette now projects causal availability without narrowing that
registry or creating another command vocabulary. Unavailable commands remain
discoverable but disabled with the missing prerequisite; active-home join and
restore, fresh-home channel/profile/role/search, offline leave, and invite copy
without an invite no longer fall through to avoidable command errors.
Form-backed commands route to the same invite, channel, profile, role, search,
peer-import, update-package, or trust-transition surface and focus the required
input; only explicit form submission supplies the semantic payload. A rendered
active/fresh-home probe covers disabled reasons and palette-to-input focus for
join, channel creation, profile editing, role creation, and retained search.
Deterministic tests cover prerequisite classification while preserving stable
command IDs. This is presentation evidence, not new command-host admission or
assistive-technology evidence.

The focused human shell has crossed an additional lived macOS gate. A fresh
ad-hoc-signed `.app` launch presented only create, signed-invite join, and
identity recovery; creating a space brought the ordinary peer service online
without a topology step. The native file dialog wrote a mode-`0600` recovery
kit and removed the persistent recovery warning only after the Rust command
accepted it. The People surface updated a profile and created a signed invite,
and the native composer carried a keyboard-authored message through Rust
admission and cleared only after success. Terminating and relaunching the exact
same bundle preserved and projected that message and automatically restored the
online service. A second fresh native home restored through the macOS file
chooser, returned online under the recovered principal on a new device, required
a new mode-`0600` recovery kit, and preserved that completed setup across a full
restart. A separate-bundle native joiner then accepted a locally transferred
signed invite, projected both members, posted from its new principal, and the
inviter received and projected the accepted message. This is local
packaged-debug evidence, not the external three-machine, Windows,
assistive-technology, or release-artifact gate.

Damaged local-home handling now preserves the recovery boundary instead of
misrepresenting unreadable state as a fresh installation. Rust distinguishes a
missing home from an existing unusable one and owns the confirmed transition:
`identity.json`, `quic-cert.json`, and the SQLite database plus its WAL/SHM
companions move into a private `.unusable-home-*` archive under the selected
home. Nothing is deleted, `product-updates` remains in place as separate update
trust state, and a healthy home is ineligible for the transition. A serialized
shell regression carries an exported recovery kit through malformed local
identity, structured failure, archival, genuinely fresh state, and restoration
of the same person-level peer identity on a newly authorized device. Native
presentation crossed a focused local macOS gate: the packaged app showed the
bounded damage explanation with collapsed technical detail, placed keyboard
focus into the explicit archive confirmation, returned to truthful onboarding
after acceptance, and focused **Recover My Identity**. This does not yet prove
recovery from a damaged home on Windows or through an external-machine field
test.

A new disposable-home run of the current ad-hoc-signed macOS `.app` exposed and
then closed a keyboard handoff gap in that same path. Native pointer activation
left the WebView document root active after `home.init`, so the vanished create
button had no meaningful focus destination. The coordinator now rejects
document roots as command origins and uses causal fallbacks. Repeating the
fresh run focused **Save Recovery Kit** after accepted creation; after the
native save dialog wrote a uniquely named kit and Rust removed the recovery
prompt, focus moved to **Message #general**. Deterministic tests cover both a
disconnected origin and a document-root origin. This is lived local macOS focus
evidence, not an assistive-technology claim.

The degraded-connection human path has crossed a focused local macOS gate. Two
separately identified ad-hoc-signed native bundles joined through a real signed
invite. After the inviter exited, an explicit refresh retained the failed
ordinary-peer synchronization as a Rust-owned health observation: the joiner
header changed from **Online** to **Online · 1 problem**, Connection & sync named
the unavailable inviter, and its retry button carried that exact peer/device
command payload. Restarting the inviter on its previously advertised endpoint
and invoking the retry cleared the broken row and restored **Online** without
changing membership or local conversation availability. This is local
loopback lived evidence, not the external IPv6 field, physical-machine, or
assistive-technology gate.

Signed-invite handoff has crossed a focused native macOS feedback gate. From a
fresh packaged launch, a real Rust-created space invite reached the native
clipboard and only then produced the visible live-region message **Signed
invite copied. Send it privately to the person you want to invite.** The
deterministic clipboard boundary separately rejects missing and failed writes,
preserves technical detail, and directs the person to manually copy the
complete signed JSON. This proves local clipboard feedback, not successful
delivery to another person or an assistive-technology announcement.

Invite admission has crossed a focused native macOS review gate. A second
fresh, separately identified packaged bundle pasted a real Rust-created signed
invite and, before submission, displayed **My Space**, stable space
and authority identifiers, the local expiry time, one included ordinary peer,
and the unbound bearer-reuse limitation. The surface explicitly labeled these
as untrusted claims and named Rust's signature, genesis, expiry, governance,
and peer-record checks. Choosing **Join Space** then completed through the real
Rust command and projected both members. Deterministic preview tests cover
expired, conflicting, partial, and oversized input without turning preview
parsing into admission. This is local packaged evidence, not external delivery,
offline-inviter, Windows, or assistive-technology evidence.

Fresh onboarding now makes a `.voxinvite` file the ordinary handoff, keeps the
raw signed JSON fallback behind explicit disclosure, and disables submission
until one of those sources contains input. A rendered fresh-state interaction
probe exercised the visible file-picker trigger, confirmed the hidden native
input does not create a duplicate accessible control, carried a selected file
into the same bounded claims review, and verified that manual text remains
keyboard-reachable. Desktop and narrow viewport checks showed no horizontal
overflow. This is rendered browser evidence for presentation and interaction,
not a new native file-dialog or successful-admission claim; the preceding
packaged gate remains the Rust-owned admission evidence.

Invite lifecycle controls have crossed a rendered interaction and Rust
operational gate. The People surface offers explicit one-hour, one-day,
seven-day, and thirty-day expiries, truthfully states bearer reuse, and lists
Rust-projected active governance invitations. Revocation opens an alert dialog,
focuses the consequential action, names ordinary-peer propagation and the
stale-partition limit, and returns focus to the exact invite row on cancel. A
command-host regression proves the semantic action removes the active invite
from the authoritative snapshot, clears the matching local handoff copy, and
remains revoked after restart. This is not packaged-native or external-network
evidence for the new controls.

Customization has crossed a focused native macOS gate. The packaged workbench
opens one human settings surface from More, presents everyday behavior before
advanced semantic-token and metric controls, and exposes contextual accessible
names for each save action. A timestamp-style change traversed the Rust-owned
preference command, survived a full restart, and projected back as the selected
value. `ui.preferences.reset` then restored the complete preference and layout
defaults through the same authority, survived another restart, and left the
full advanced ontology reachable. A reconciler regression test also proves
that changing a control's semantic action replaces the node carrying its old
listener instead of pairing a new label with stale behavior. This remains
packaged-debug macOS evidence, not an external assistive-technology or release
artifact gate.

The public Discord families have crossed their operational gate. Space
governance now carries channel definitions, profiles, roles, permissions,
bans, and invites. Signed room events carry posts, edits, redaction tombstones,
reactions, mentions, threads, pins, and content-addressed attachments. SQLite
retains the accepted facts; multi-room QUIC anti-entropy forwards them through
ordinary peers; the app projects unread cursors, mention notifications, and
local full-text search. A serialized two-home test exercises those commands
through the same host used by the native shell. That serialized path now also
opens a retained search result through `message.open`, where Rust validates its
channel membership and returns a bounded projection anchored on the exact
event. The same path serves mention notifications, so older results do not
silently disappear behind the ordinary latest-500 projection and the frontend
does not reconstruct retained history. Rendered-preview evidence confirms that
the notification affordance routes through `message.open` and refuses to
simulate acceptance. This is semantic and rendered evidence, not a new
packaged-native search claim. The serialized path also
projects admitted role assignments and ban state, proves that a banned member
loses current membership and role assignment, and proves that unbanning only
permits a future invited rejoin. The packaged macOS People surface presents
human permission names and member/role actions while keeping stable IDs in
command payloads; private-channel creation selects current named members and
explains the self-only case without requiring principal-ID entry.

File sharing now has a human consent boundary before admission. Selecting a
file opens a focused review of its name, type, decoded size, destination,
projected audience, and durable-retention limitation; cancel returns to the one
visible attachment affordance without publishing bytes. The Rust projection
reports decoded size from the already-admitted content, while the serialized
two-home path proves hash/size projection, convergence, search, and an admitted
attachment tombstone. Core admission now permits an attachment author or
authorized moderator to create the same `MSG_REDACT` fact the projection
already understood. The P2P draft records the existing standalone
`ATTACHMENT_ADD` wire shape. Rendered evidence covers review focus, cancellation,
file-specific actions, and refusal to simulate preview acceptance; this is not
new packaged-native file-dialog or external-machine evidence.

Manual multi-peer verification no longer silently targets the first stored
availability record. Connection & sync and the re-entrant Field Test view share
a disposable explicit target selection, show its address/principal/device
tuple, and send that exact principal/device payload through the existing Rust
diagnose or sync command. Field-test completion is evaluated against the
peer-named activity result for the selected target rather than any prior peer
success. Deterministic selection tests and rendered two-peer evidence cover
target retention, fallback when a record disappears, keyboard focus, visible
identity, and command payloads. This does not add routing or membership
authority, and it is not non-loopback or three-machine evidence.

The no-peer recovery action is now causally complete. A context-free
`peer.import` from the palette, Network Health, or Field Test opens and focuses
the shared Connection & sync availability review rather than submitting an
empty frontend draft. The bounded preview exposes claimed label, address,
principal, device, and space while repeatedly naming those claims as untrusted;
malformed or incomplete current-format JSON cannot be submitted. Rust still
validates and stores the complete record, and the existing sync authority check
still refuses a foreign-home record. A successful import selects that exact
principal/device tuple for manual checks and optional auto-sync. Deterministic
preview/selection tests and rendered palette-to-review evidence cover malformed
input, foreign-space warning, focus, draft preservation on refusal, and refusal
to simulate preview acceptance. This is not accepted native import or external
network evidence.

The member-ban affordance now requires an explicit confirmation that explains
loss of participation authority, retained history, and the fresh-invite
requirement before it invokes `member.ban`. Rendered-preview evidence exercises
confirmation focus, cancellation back to the stable member row, and refusal to
simulate the semantic command. The serialized two-home test above remains the
authority evidence for the accepted ban transition; this is not yet a new
packaged-native or assistive-technology claim.

Role assignment is no longer a one-click authority change. Grant and revoke
affordances now focus an explicit confirmation naming the member, role,
direction, and human-readable permissions gained or lost, while noting that
other roles remain unchanged. Cancel and completion return keyboard focus to
the stable role row. Rendered-preview evidence covers the confirmation and
semantic-command route; the serialized two-home governance path remains the
accepted role-assignment evidence, so this is not a new packaged-native or
assistive-technology claim.

The shared shell error contract now distinguishes `needs_input` from authority,
home, reachability, synchronization, human-intervention, and internal failures.
Representative serialized and inhabitant-surface tests prove that an empty
retained-message search is returned as correctable input while malformed command
schemas and unsupported commands remain internal integration errors. The Rust
classifier narrowly covers authoritative validation outcomes for message text
and mentions, reactions, attachments, profiles, channel and role creation, and
search; it does not reclassify permission failures or infrastructure faults.
Invite joining now preserves the same distinction: an observed revocation,
expiry, or malformed signed invite asks for corrected input; an already-used
home names the separate-fresh-home requirement; and an unexplained local join
failure remains internal rather than being mislabeled as reachability. A
serialized revoked-invite test proves both the human recovery copy and that the
refused join leaves the destination genuinely fresh.

The packaged macOS conversation surface has crossed a lived reply/edit gate. A
person selected Reply on a retained message, received named composer context,
posted through `message.send` with the authoritative thread root, and observed
both the reply annotation and incremented root count. The same run opened an
inline edit, focused the native text control, saved through `message.edit`, and
exercised Escape cancellation without a browser prompt. The edited root and
thread reply survived a full restart of the exact same artifact. Existing
serialized two-home evidence continues to prove convergence through those same
commands; this lived run is local packaged-debug evidence.

The human composer and inline editor now expose a named member picker for
mentions. Rendered-preview evidence proves that selecting a member inserts the
visible name and returns focus to the text control; deterministic composition
tests prove unambiguous typed-name resolution and duplicate-name selection; and
the serialized two-home command test carries the resulting stable peer ID into
the admitted message and recipient notification. This is local semantic and
rendered evidence, not a new packaged-native or assistive-technology claim.

The same packaged surface now derives reaction and pin actions from projected
accepted state: a native run added and removed the local reaction and pinned
and unpinned the message through their distinct semantic commands. Deletion no
longer shares that one-click toggle shape; it focuses an explicit confirmation
that explains the retained signed tombstone, with a separately exercised cancel
path.

Private channels have crossed their first confidentiality and recovery gate.
Each member publishes a signed X25519 encryption key, private channel creation
wraps a random epoch key independently to each admitted member, and every
private room fact is carried inside an authenticated encrypted envelope. A
decrypted inner fact is revalidated by the ordinary semantic authority before
projection. A three-home test proves that the excluded peer neither lists nor
stores the private room, retained events and local key files lack the message
plaintext, admitted peers decrypt successive epochs, and a fresh recovered
home restores the epoch keys and history from an ordinary peer.

The private-channel rotation affordance now projects the admitted epoch and
current private-member count from Rust and requires an explicit confirmation.
The copy names the forward-only confidentiality effect and refuses to imply
remote erasure of earlier ciphertext, keys, or plaintext. Rendered interaction
evidence proves alert-dialog focus and stable-row focus return; the existing
three-home confidentiality regression now also proves that the projected epoch
advances through admitted governance and reconstructs after restart. A
serialized-shell regression carries `channel.rotateKey` through the shared
semantic host and observes that same durable projection. This is not
packaged-native, physical-participant, or external-network evidence.

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
permission-denial truthfulness. The packaged macOS surface now presents mutually
exclusive pre-join and in-call actions, explains the direct four-person envelope
before capture, localizes actionable permission and device failures, and names
each participant's direct connection state for assistive technology. This
verification machine has no physical camera or microphone, so capture,
permission prompts, two-device media flow, and the resulting in-call tiles are
not claimed as lived local evidence.

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
reachability, actual assistive-technology operation, and physical media devices
remain the external gates already named in the evidence horizon, not locally
completed claims. The release-bound beta receipt now requires those human
external gates explicitly, so local mocks and semantic tests cannot silently
stand in for lived evidence.
