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

The authenticated beta-evidence status command may expose every incomplete
receipt section in one pass, but it establishes only receipt completeness and
internal consistency. It does not elevate operator attestations into observed
or cryptographic proof of the lived causal paths.

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

The human recovery handoff now follows that authority result instead of falling
through silently to conversation. A successful restore announces that authority
from previous devices was revoked and focuses the fresh recovery-kit action;
status dismissal preserves the same fallback. The Identity Recovery view shows only
the locally recorded save time and a fresh-kit renewal action, never the bearer
file path or bytes. Deterministic presentation regressions cover the handoff on
top of the existing serialized principal-continuity, revocation, restart, and
ordinary-peer recovery evidence; external assistive-technology operation
remains a separate beta gate.

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

The update view and command palette now derive the same pre-invocation
availability from the Rust-projected product-generation snapshot. Missing trust
roots disable discovery, staging, manual install, and trust rotation with an
explicit reason; an undiscovered release disables staging; a missing staged
generation disables activation and discard; and missing verified history
disables rollback. These checks prevent avoidable failing actions but do not
authorize an update: the unchanged Rust path still authenticates every release,
transition, sequence, and generation change. A fresh-home rendered preview
showed each unavailable palette command with its specific prerequisite, while
the Product Updates surface exposed disabled discovery, install, and trust
actions with the missing-root description. This is rendered unavailable-state
evidence, not authenticated update or packaged-native evidence.

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
without an invite no longer fall through to avoidable command errors. A
rendered fresh-home probe exposed that `message.composer.focus` was the remaining
context-only exception: it closed the palette and found no composer. The shared
availability decision now classifies it with the other home-dependent commands;
both palette search and its direct shortcut keep it visible, disabled, and
described as requiring create, join, or recovery first.
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
The pre-archive safety review now owns modal keyboard focus: both Tab directions
wrap within Archive and Cancel, Escape cancels, and focus returns to the stable
prepare action even though opening the review removes the original DOM node. A
read-only damaged-home preview and rendered interaction probe cover the dialog
semantics and cancellation path without manufacturing a successful archive;
the serialized Rust recovery regression and packaged macOS run remain the
authority and accepted-transition evidence.

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

Global error and success notices now preserve causal keyboard location across
render reconciliation. Correctable validation returns to the invalid control;
command failure dismissal reacquires the initiating semantic action inside an
open transient surface, with composer/header fallbacks only when that action no
longer exists. A rendered preview probe exercised an empty channel name and a
preview-rejected signed-invite command. In both cases the alert cleared and
focus remained in the causal workflow. This is rendered WebView-equivalent
evidence, not native assistive-technology or successful-command evidence.

Channel navigation now preserves that causal location when acceptance changes
the control topology. The Rust-selected channel row exposes
`aria-current=page`, and a completed `channel.select` moves focus from the
removed Select button to that surviving row. The frontend neither predicts the
selection nor marks it current before the returned snapshot. Deterministic
source and rendered accessibility-tree evidence cover this handoff; actual
assistive-technology operation remains part of the external human gate.

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

The broader workbench narrow-window projection has also crossed a rendered
browser gate. At 640, 480, and 360 CSS pixels, the current conversation and
composer remain fully reachable without document-level horizontal scrolling;
the header wraps its actions inside the visible width. At 480 pixels,
Connection & sync remains inside the scrollbar-safe containing width and has no
internal horizontal overflow. The compact projection stacks the existing named
dock areas without changing their stable IDs, persisted placement, visibility,
or command authority. This is rendered presentation evidence, not
packaged-native, Windows, or assistive-technology evidence.

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

The customization frontend now distinguishes projected values from in-progress
human drafts: unchanged fields expose disabled contextual Save actions, edits
enable only their associated Save action, and drafts survive ordinary refreshes
and reset-review cancellation without becoming a second persistence authority.
Reset All Customization opens a modal review that names appearance, spacing,
behavior, and workbench placement or visibility, states which protocol state is
untouched, focuses the consequential action, traps keyboard navigation, and
returns to Reset after button or Escape cancellation. Only explicit confirmation
invokes the existing Rust-owned reset command; accepted preference or reset
commands clear their matching disposable drafts. The distinct
`workbench.layout.reset` command now uses the same modal discipline but names
only dock placement and visibility, explicitly preserves appearance, spacing,
and behavior, and returns cancellation to the exact reset control or the visible
More entry point when no narrower reset control is rendered. A rendered preview probe
verified unchanged/changed/reverted Save states, draft survival across Refresh,
the reset review and initial focus, both cancellation paths, retained draft on
cancel, and focus return without invoking reset. This is rendered presentation
evidence. A fresh-home rendered palette probe separately verified the
layout-only review, its bounded preservation language, consequential initial
focus, Escape cancellation, and focus return to collapsed More without invoking
the command. These are not new packaged-native or assistive-technology claims.

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

Governance creation surfaces now distinguish disclosure from admission in their
accessible names. **Open channel creation form** and **Open role creation form**
only expose retained frontend drafts and expanded state; the nested **Create
Channel** and **Create Role** controls remain the sole semantic submissions to
Rust. Deterministic source checks and a rendered preview verified one distinct
opener and submit action for each form. This is rendered interaction evidence,
not admitted-governance, packaged-native, or assistive-technology evidence.

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
Before reading bytes, attachment selection now applies the shared advisory
short-text check to filenames with Rust's 255-Unicode-character bound and gives
rename guidance that states nothing was shared. Unusable browser MIME metadata
is normalized to the already-supported `application/octet-stream` label in the
review instead of creating a locally unfixable failure. Deterministic helper and
product-source regressions cover ordering and normalization. Rust still validates
the original filename, projected MIME, decoded bytes, size, and hash; this is
not new rendered or packaged file-dialog evidence.

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

The zero-peer degraded state no longer offers targetless diagnosis or sync as
though it could complete. Palette and Network Health actions share the
Rust-projected known-peer prerequisite, remain disabled with **Join with an
invite or import peer availability first**, and leave Import Peer available as
the causal recovery path. The existing Rust commands still validate the exact
principal/device payload; the frontend check neither imports a route nor grants
authority. Deterministic availability and source tests plus a rendered temporary
zero-peer fixture verified the disabled actions and descriptions in both
surfaces while Import Peer remained available. This is rendered unavailable-state
evidence, not accepted import, reachability, synchronization, or external-network
evidence.

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

Advanced Bind and Advertise input now presents the complete IPv6 socket shape
with a port instead of ambiguously asking for an address. One advisory helper
accepts empty automatic defaults or bracketed IPv6 sockets, including numeric
scope IDs and IPv4-mapped tails, while correcting edge whitespace, controls,
malformed addresses, missing ports, and ports above 65,535. **Go Online** opens
Connection & sync, marks and focuses the exact invalid field, and does not
invoke the semantic command until both drafts are locally usable. Rust's typed
`SocketAddr` deserialization and service startup remain authoritative for the
accepted configuration. Deterministic helper, product-source, and native bundle
tests cover this presentation path; this is not new rendered, packaged-native,
non-loopback, or field evidence.

Runtime palette actions now reflect the projected transition rather than
offering contradictory Start and Stop choices. While online, context-free Go
Online is disabled with a route to Connection & sync for explicit Bind or
Advertise reconfiguration, while Go Offline remains available; the visible
in-form Go Online action remains available to apply those drafts. Offline state
reverses the palette availability. The semantic command and Rust service
configuration authority are unchanged. Deterministic availability tests and a
rendered online preview verified the disabled palette reason and the still-active
Connection & sync action. This is rendered presentation evidence, not a service
restart or external reachability claim.

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

Invitation revocation, private-channel key rotation, member bans, role
assignment, message or attachment deletion, and attachment sharing now reuse
one modal interaction boundary without merging their semantic commands. Every
review blocks background interaction, focuses its consequential action, wraps
Tab in both directions, suppresses unrelated shortcuts, cancels with Escape,
and retains its existing stable return target. Deterministic source coverage
holds all six review families to that boundary. A rendered invite-revocation
probe verified the overlay, alert-dialog identity, both focus wraps, shortcut
containment, Escape cancellation, and stable invite-row return. This is shared
human-presentation evidence; Rust admission, retained facts, and command-specific
authority remain unchanged, and no packaged-native or assistive-technology
claim is added.
Rejected actions no longer put the sole structured error behind a still-open
blocking review. Consequential, damaged-home, customization-reset, and
product-update modals own their error while open: the alert retains bounded
human recovery and technical detail, focuses **Dismiss**, and returns to the
same retry action without closing the review or claiming acceptance. A rendered
preview refusal exercised this complete failure, dismissal, and retry-focus
path for invitation revocation. This is modal failure-presentation evidence,
not a native governance rejection or admitted-fact claim.

The shared shell error contract now distinguishes `needs_input` from authority,
home, reachability, synchronization, human-intervention, and internal failures.
Representative serialized and inhabitant-surface tests prove that an empty
retained-message search is returned as correctable input while malformed command
schemas and unsupported commands remain internal integration errors. The Rust
classifier narrowly covers authoritative validation outcomes for message text
and mentions, reactions, attachments, profiles, channel and role creation, and
search; it does not reclassify permission failures or infrastructure faults.
The authenticated inhabitant SSE surface now wakes resident agents when the
Rust-owned snapshot changes. A successful HTTP semantic command emits a
process-monotonic `snapshot.changed` notice only after the command host returns
its new snapshot; asynchronous peer-service invalidations use the same channel.
The notice carries the canonical snapshot URL, so agents re-read authoritative
state instead of relying on an HTTP-side reconstruction of room, governance, or
recovery meaning. Unit evidence covers multi-subscriber monotonic delivery, and
an isolated live HTTP/SSE rehearsal carried `home.init` from an authenticated
command response showing initialized identity to sequence 1 on an already-open
event stream. This is local loopback agent-surface evidence, not a resident
Watch/WFB integration or autonomous-agent claim.
Episodic agent actions no longer require repository-source inspection to learn
their payload shape. The same Rust-projected `UiCommand` records used by the
WebView now name each shell command's request DTO, while empty-payload and
frontend-only commands remain explicit through scope and a null payload type.
Authenticated inhabitant discovery exposes the TypeScript contract generated
from those Rust DTOs. Contract equality plus command-to-type completeness tests
prevent the WebView and inhabitant descriptions from drifting. An isolated live
rehearsal discovered `home.init` as `InitHomeRequest`, retrieved that declaration
from the advertised contract URL, submitted the typed payload, and received an
initialized authoritative snapshot. Payload declarations are affordances, not
validation or protocol authority.
Inhabitant action results now attribute only the Rust-owned service activity
created during that serialized HTTP command. The adapter captures the host's
monotonic activity cursor, invokes the ordinary semantic command, and filters
the returned snapshot or failed-command activity by that cursor. A sidecar
command gate prevents concurrent agent calls or snapshot refreshes from
claiming one another's rows;
it does not alter the host's command serialization or admission path. An
isolated authenticated HTTP rehearsal showed `home.init` returning only its
initialization and service-start rows, the following `runtime.goOffline`
returning only the later stop row, and an unsupported command returning
structured `internal_error` with no inherited activity. This proves local
causal result visibility, not external-agent judgment or autonomous recovery.
Initial snapshot failure no longer ends in a static dead-end screen after
advising the person to retry. The pre-component shell renders the same bounded
structured explanation, states that retry does not delete, archive, or replace
local state, focuses **Try Again**, and repeats only the native `shell.refresh`
request after explicit activation. Technical details remain keyboard-operable
and report expanded state. A deterministic retry-loop test covers repeated
failure before first success; a temporary clean-origin preview failure verified
the focused error, recovery description, Enter-opened details, explicit retry,
and eventual ordinary product load. This is rendered startup recovery evidence,
not damaged-home archival, packaged-native credential, or identity-recovery
evidence.
The profile, channel, and role forms now make their narrower frontend
prerequisite checks causally usable: empty profile display name, empty channel
name, empty role name, and missing role permissions each mark and describe the
exact control or group and return keyboard focus to it. The accessible field
name remains separate from the error description. Editing that exact field or
selecting a missing permission removes the stale inline and global error
immediately; unrelated
edits and later non-validation failures cannot inherit or erase a field marker
accidentally. This presentation check does not accept the command or replace
the Rust classifier and semantic admission path.

Those three human-name forms now also share one testable advisory mirror of the
Rust-admitted short-text shape: no leading or trailing space, no control
characters, and at most 80 Unicode characters. Rendered probes focused and
described a leading-space profile name, an 81-character channel name, and a
trailing-space role name. The helper is included in both preview and builtin
product-component assembly, with a Rust bundle regression proving the native
source contains the helper and all three consumers. This remains corrective
presentation; Rust independently decides admission and protects replay paths.
The optional profile About and channel Topic fields use the same helper family
for their admitted 512- and 1,024-Unicode-character bounds and control-character
exclusion. Rendered probes focused and described 513- and 1,025-character emoji
or text drafts respectively; empty optional values remain permitted.
Invite joining now preserves the same distinction: an observed revocation,
expiry, or malformed signed invite asks for corrected input; an already-used
home names the separate-fresh-home requirement; and an unexplained local join
failure remains internal rather than being mislabeled as reachability. A
serialized revoked-invite test proves both the human recovery copy and that the
refused join leaves the destination genuinely fresh.

Signed invite creation no longer silently clamps a semantic caller's requested
authority window. Rust accepts the documented finite envelope of 1 minute
through 30 days and returns correctable-input recovery outside it before
creating the signed governance fact. The human form retains its bounded
one-hour, one-day, seven-day, and thirty-day choices, now reviews the selected
bearer-capability window in a live region, and labels the action **Create signed
invite**. Serialized semantic-command tests cover refusal on both sides of the
range and prove no active invite was admitted; a deterministic product-source
regression covers the review copy. A rendered
active-home probe changed the choice from 24 hours to 7 days and observed the
exact live 7-day review with the named creation action still available; it did
not simulate command acceptance. This is not new packaged-native, admitted
governance, multi-peer, or assistive-technology evidence.

The packaged macOS conversation surface has crossed a lived reply/edit gate. A
person selected Reply on a retained message, received named composer context,
posted through `message.send` with the authoritative thread root, and observed
both the reply annotation and incremented root count. The same run opened an
inline edit, focused the native text control, saved through `message.edit`, and
exercised Escape cancellation without a browser prompt. The edited root and
thread reply survived a full restart of the exact same artifact. Existing
serialized two-home evidence continues to prove convergence through those same
commands; this lived run is local packaged-debug evidence.

The ordinary composer and inline editor now prevent a locally knowable empty
submission before invoking authority. One shared predicate disables **Send
Message** or **Save changes** for empty drafts, leading or trailing whitespace,
null characters, and more than 4,000 Unicode characters; it is reused by typing,
Enter, member-picker insertion, and form submission. The displayed counter uses
Unicode code points rather than UTF-16 units. Rendered probes observed `2 / 4,000`
for two emoji, inline edge-whitespace guidance, retained over-limit text with
`4,001 / 4,000`, enabled visible text and `@Bob` insertion, and keyboard clearing
back to disabled in both paths. Rust still owns
message length, authorization, mention, and semantic admission; this is
presentation evidence, not accepted-message evidence.

Retained search now has one Rust-owned finite request shape for both human and
agent callers: after edge whitespace is trimmed, a query must contain 1 to 1,024
Unicode characters and no control characters. The semantic command returns
correctable-input recovery for each refusal before lowercasing or scanning
retained messages. The frontend advisory mirror disables **Search Messages**
and gives live correction guidance for the same locally knowable shape while
preserving edge whitespace that Rust accepts. Serialized semantic-command tests
cover oversized and control-character refusals, and deterministic helper/source
tests cover the human projection. The prior rendered probe covers only the
empty, whitespace, visible-term, and cleared transitions; this is not a new
rendered, packaged-native, or assistive-technology claim. Rust remains the
authority for query bounds, accessible index scope, and returned retained facts.

The human composer and inline editor now expose a named member picker for
mentions. Rendered-preview evidence proves that selecting a member inserts the
visible name and returns focus to the text control; deterministic composition
tests prove unambiguous typed-name resolution and duplicate-name selection; and
the serialized two-home command test carries the resulting stable peer ID into
the admitted message and recipient notification. This is local semantic and
rendered evidence, not a new packaged-native or assistive-technology claim.

Repeated workbench controls now carry their visible target into the accessible
name. Channel selection names the public or private channel, private-key
rotation names its channel, and each message-action disclosure names the author
plus a bounded text or attachment preview. Member actions name the member, role
assignment names the role, invite revocation names the displayed expiry, and a
visible reaction or pin action names the exact author/content context. A
rendered accessibility-tree probe covers public text, mentioned text,
attachment-only messages, and the populated People surface while the
underlying controls continue to invoke the existing stable commands and
payloads. Escape from that
nonmodal panel closes it, restores focus to its People invoker, and projects the
collapsed state. Deterministic source regressions preserve the contextual
labels and preview bound. This reduces nonvisual navigation ambiguity but is
not the actual assistive-technology beta gate.

Duplicate member display names no longer collapse those governance targets back
into position-dependent labels. One case-insensitive helper leaves unique names
unchanged and appends a bounded principal-derived member marker of at least 12
characters only when names collide. When 12 characters still collide, the
marker expands to the shortest unique principal prefix. The same label appears
on the member card, mention
choice, Ban/Unban affordance and confirmation, and role assignment affordance
and confirmation, while each command still carries the complete stable peer ID
to Rust. Deterministic helper, product-source, and native bundle tests cover the
mapping. A rendered case-insensitive duplicate-name fixture probe observed
distinct matching member markers on both cards, the mention choice, Ban review,
and role-assignment review, and did not invoke either governance command. This
is not admitted-governance, packaged-native, or assistive-technology evidence.

Roles now retain the parallel distinction allowed by governance: duplicate
case-insensitive names leave unique names uncluttered but append the shortest
unique stable role-ID suffix of at least eight characters when names collide.
The marker persists from the role card through Manage members, Grant/Revoke,
and the focused assignment review, while Rust still receives the complete role
ID and independently decides authority. Deterministic helper, product-source,
and native bundle tests cover the mapping. A rendered case-insensitive
duplicate-role fixture probe observed distinct role cards and Manage members
controls, then carried the selected marker and its distinct permission summary
through the assignment review without invoking governance. This is not
admitted-governance, packaged-native, or assistive-technology evidence.

Channels now preserve that distinction across navigation and conversation when
case-insensitive names collide. One helper leaves unique names uncluttered and
appends the shortest unique room-ID suffix of at least eight characters to the
channel card, Select and private-key rotation paths, selected header, timeline,
composer target, retained-search results, and notification actions. Commands
and projections still carry complete room IDs, and the frontend does not infer
room authority from the label. Deterministic helper, product-source, and native
bundle tests cover the mapping. A rendered public/private case-insensitive
duplicate-name fixture probe observed distinct channel cards, selected header,
timeline, composer target, private Select and key-rotation controls, the focused
rotation review, and the notification action; it did not invoke either room
command. This is not admitted-channel, packaged-native, or
assistive-technology evidence.

Active invite revocation no longer collapses multiple bearer capabilities with
the same human-formatted expiry into identical actions. The frontend compares
the displayed expiry labels and, only on collision, appends the shortest unique
invite-event-ID suffix of at least eight characters to the row action, focused
confirmation, and final Revoke button. The full invite event ID remains visible
in the row and is the sole governance command payload. Deterministic helper,
product-source, and native bundle tests cover the mapping. A rendered
same-expiry fixture probe observed two distinct row actions, carried the chosen
marker through the focused confirmation and final Revoke button, and returned
to both unchanged rows after cancellation without invoking governance. This is
not admitted-governance, packaged-native, or assistive-technology evidence.

Repeated identical posts no longer collapse message controls back into
position-dependent labels. The frontend compares the existing bounded
author/content or attachment context among visible messages and, only on
collision, appends the shortest unique message-event-ID suffix of at least
eight characters. That label persists across reaction chips and actions,
downloads, Message actions, Reply, Edit, Delete, and the focused deletion review.
Every command still carries the complete target event ID, and Rust remains the
message authority. Deterministic helper, product-source, and native bundle tests
cover the mapping. A rendered duplicate-own-message fixture exposed two distinct
action disclosures, carried the selected suffix through Reply, Edit, Delete,
the focused deletion review, and its final Delete button, then returned after
cancellation without invoking the command. This is not admitted-message,
packaged-native, attachment-collision, or assistive-technology evidence.

Accepted reaction and pin toggles also preserve causal keyboard location
without preserving a stale command listener. Reconciliation replaces the
control when the Rust-projected action changes from add to remove or back; a
distinct presentation-only key then focuses the exact successor. Removing the
last visible reaction falls back to the stable message row when no chip
survives. Deterministic regressions cover the replacement and contextual
accessible names; actual assistive-technology behavior remains in the external
human gate.

Disclosure controls have crossed a broader rendered accessibility gate. More,
onboarding fallbacks, profile and identity details, customization, signed
artifact entry, peer setup, invitations, channel creation, member and role
management, message actions, error details, and the mention picker all use one
owner-independent presentation helper. It exposes a button role, synchronizes
`aria-expanded` with the native `details` state, and preserves both Enter and
Space activation. Rendered traversal covers the default workbench plus People,
Customize, and Product Updates, including expanded-state changes. This remains
browser accessibility-tree and keyboard evidence, not an actual VoiceOver,
Narrator, or NVDA claim.

A rendered private-channel probe exposed that blanket disclosure preservation
also blocked intentional product-driven closure: turning private mode off
cleared the checkbox while leaving its now-inapplicable options expanded. The
reconciler now preserves `open` only for native user-owned disclosures and
honors it for explicitly controlled profile, peer-import, channel, privacy,
role-member, and role-create forms. Unit coverage proves both sides of that
boundary, and the repeated probe observed `{checked:false, open:false}`. This is
rendered presentation evidence, not an authority or assistive-technology claim.

Fresh onboarding has also crossed a rendered system-color and narrow-window
gate. At 420×700 it keeps create, signed-invite join, and recovery in one
vertical causal path without document-level horizontal overflow; the manual
invite disclosure remains keyboard-operable. A dark-scheme render exposed that
fixed light message surfaces and fixed green/red status foregrounds did not
adapt with `CanvasText`. The stable semantic token IDs now retain user
customization while their defaults derive message surfaces from system colors
and select distinct light/dark status colors. On the rendered dark canvas, the
ownership/status and recovery-warning foregrounds measure 10.36:1 and 8.21:1
contrast respectively. This is preview rendering evidence, not packaged-native
or actual assistive-technology evidence.

The populated active-home surface has now crossed the matching 420×700 rendered
gate. The header, selected conversation, messages, composer, and direct-media
actions remain in one vertical projection with equal document client and scroll
widths. The compact header now groups its six routine surfaces into two bounded
columns instead of six separate rows. A 320×700 discriminating probe then
exposed an independent intrinsic-width overflow in the composer controls; they
now use the same bounded two-column projection. Repeated probes observed equal
document client and scroll widths at both 320px and 420px, with matching composer
client and scroll widths. Opening People exposed that the prior fixed 104px
panel offset covered the dynamically wrapped header while leaving its actions
focusable. Compact Connection and utility panels now become viewport-contained
modals, report `aria-modal=true`, and trap Tab within visible controls. The shared focus order
keeps native disclosure summaries but excludes their collapsed descendants; a
rendered Shift+Tab probe wrapped from **Close** to **Manual peer setup**. Desktop
widths retain the existing nonmodal panels and command paths. This is rendered
preview evidence, not packaged-native or actual assistive-technology evidence.

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
surface now also receives each active participant's accepted camera intent from
the Rust projection. A rendered capacity probe verifies that four projected
participants disable both visible join choices and the palette Join command
with an explicit full-call reason. A separate already-joined probe verifies that
the visible surface exposes only in-call controls while palette Join remains
disabled as redundant. These are presentation guards over projected state;
Rust remains the admission authority if that state changes. Heartbeats preserve
the latest admitted join mode, and a
remote voice-only participant renders as an explicit voice tile instead of an
empty video element; a rendered preview probe exercised that label together
with the independent **Connecting directly** state. Camera intent no longer
creates a blank video element before that direct connection reaches
`connected`: connecting and failed peers retain explicit placeholders, and a
transiently disconnected tile distinguishes ongoing reconnection from the
terminal failed tile's leave-and-rejoin action. Recovery remains attributed to
each participant rather than a global notice, including when multiple direct
connections degrade. Deterministic projection tests cover connected,
connecting, disconnected, and failed camera intent, while rendered previews
exercise the connecting and interrupted camera-intent tiles.
This does not move
participant selection, media authorization, or connection truth into the
frontend. The verification machine has no physical camera or microphone, so
capture, permission prompts, two-device media flow, and the resulting in-call
tiles are not claimed as lived local evidence.
Active-call operation now includes a stable `call.microphone.toggle` frontend
command shared by the visible call control and command palette. It changes all
local WebView audio tracks together, projects **Microphone on** or **Microphone
muted** in the local tile, and gives a leave-and-rejoin recovery action when a
restart-like call snapshot has no captured track. Deterministic track tests
exercise both directions and missing capture; rendered preview evidence covers
the muted state, recovery copy, and palette route. This local device control
does not change accepted call participation or signaling and is not physical
audio evidence.
Active camera control follows the same visible-control and command-palette
vocabulary through `call.camera.toggle`. It disables or reenables only an
already negotiated local camera track, while a signed, semantically admitted
`CALL_MEDIA` fact updates the projected camera intent seen by ordinary peers.
Only an active call participant may publish that fact, and it neither extends
liveness nor changes call membership. A voice-only capture remains explicit and
directs the person to leave and rejoin with camera instead of pretending a new
track was negotiated. Deterministic authority, serialized two-home, device-track,
and palette tests cover this path; physical camera behavior remains an external
human evidence gate.

The current local verification pass is green for the changed system
authorities: 121 Rust workspace tests, 135 frontend behavior tests, strict
`voxelle-app` lint, generated-contract equality, IPv6 QUIC startup, retained
artifact inspection, and the authority-specific recovery, invitation,
governance, private-room, media, update, release-evidence, CLI, inhabitant, and
native-host paths all pass. The current universal macOS build and its DMG
inspection are recorded separately below so artifact-specific claims remain
distinguishable from semantic test evidence.

Source commit `0559f982c5088dac7ad3b55d5020396e89578e99` now has one locally
assembled `v0.1.0-beta.4` candidate at sequence 4. Its signed manifest
authenticates the universal macOS DMG as
`c84e6b3eaefccb89b9a0af9ba0992212da3160f14697de361fc7ce05ca9dee22`,
the Windows x86-64 NSIS installer as
`a513c04d7c36c73c14bda3a41b5bb9f0399bbfca987ce7d1d980c71779ce5f28`,
and the live product generation as
`1562ea99a4fa09edbaa1ac83645ad514b77c9cb2c920e470fac1eee26c88f68d`.
The release verifier accepted all three exact bytes. `hdiutil verify` accepted
the DMG; inspection of its mounted app reported `x86_64` and `arm64`, a valid
ad-hoc signature, and hardened runtime. The cross-built Windows executable is
COFF x86-64 with the Windows GUI subsystem, but has not run on Windows.

From the exact candidate DMG, an isolated native rehearsal using the documented
test-file vault displayed only Create, signed-invite Join, and Recover; accepted
Create through Rust, immediately projected the offline recovery-kit obligation,
showed online state, admitted `beta4-release-bound-rehearsal`, and projected it
in Conversation. The app exited cleanly, the read-only image detached, and the
disposable home was removed. This is release-bound packaged-WebView and local
admission evidence, not production Keychain, restart, invite, recovery,
multi-peer, physical-media, or completed release-receipt evidence. The generated
receipt template still fails distribution, Windows, field, human, and custody
sections exactly as intended; no tag or public beta.4 release is claimed.

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

Human-gate recording no longer requires hand-editing its nested receipt
section. The release CLI copies the latest partial `voxelle-beta-evidence/v1`
receipt, requires an explicit flag for every keyboard-only assistive and
physical-media observation, validates the complete human section against the
field roles, preserves other evidence sections, and refuses output overwrite.
An end-to-end disposable receipt proves successful recording and fail-closed
omission without claiming the synthetic values as lived evidence. The command
records operator attestations; it does not observe or strengthen them.

The three-machine field gate has the same staged recording boundary. Its
release command requires distinct A/B/C machine, principal, and device values;
real IPv6 listen and advertised sockets; exact per-author markers;
bidirectional A/B diagnosis and synchronization; offline-inviter forwarding;
retained history; and three-way message visibility. Existing field validation
runs before the section is replaced, so loopback or otherwise invalid topology
cannot create an output receipt. Successful disposable recording is tooling
evidence only, not a non-loopback network claim.

Distribution evidence is staged through an authenticated boundary rather than
hand-edited. The release CLI verifies the downloaded signed manifest, derives
its exact release tag URL, checks that the partial receipt identifies the same
release and sequence, and requires separate observations for public readback,
DMG verification, universal binary inspection, packaged launch, live
activation, rollback, and current-generation reactivation. This binds the
record to release identity without claiming the recorder performed any of
those lived operations.

Custody recording likewise authenticates only public release material. It
derives distinct ordinary-release and recovery-only key IDs from the reviewed
capability roles, requires separate non-secret storage descriptions plus
separate-protection, offline, development-copy-removal, and restore-test
attestations, and validates before staged replacement. The recorder never
reads, moves, unmounts, or deletes signing secrets; destructive removal remains
an explicit operator action after recoverability is established.
