# Work Frontier

## Outcome

Voxelle is ready for a credible beta when people and agents can install or
attach to the same product, join the same spaces, and sustain understandable
coordination through identity, invitation, communication, recovery,
customization, and small-group media. Beta claims remain bounded by the
truthful system contract and require proportional native, multi-peer,
platform, accessibility, agent, and release evidence.

## Goal invariants

- **Human goals precede machinery.** Ordinary success presents conversation,
  people, and invitation before topology, protocol, or workbench controls;
  deeper controls remain available for customization and degraded operation.

- **One authority and conversation model serves people and agents.** Human and
  agent affordances use the same stable semantic commands, accepted facts,
  rooms, threads, permissions, retention, and synchronization. No hidden
  agent-only task product may become coordination truth.

- **Resident delivery is durable and independently checkpointed.** Each
  authenticated origin owns its observation consumer and resumes uncommitted
  changed threads after disconnect, crash, or restart. Human read state,
  sibling cursors, signed acknowledgement, handling, SSE sequence, and local
  fact sequence remain distinct meanings.

- **Coordination claims stay literal and causal.** Origin provenance says only
  which device-certified surface submitted a fact. Address hints prioritize an
  origin without granting assignment, obligation, presence, visibility, or
  authority. Continuations and acknowledgements remain participant assertions;
  causal maxima, exact reply edges, and result bindings—not timestamps—derive
  actionability.

- **Co-resident attention is independent below shared principal authority.**
  When several human or resident origins share one principal, one origin's
  decline, release, handling, or explanatory reply must not suppress or
  fabricate another origin's directed pending attention. Principal authorship
  and protocol permissions remain shared; local routing and resumption remain
  origin-specific. Two source-blind decline/handoff rehearsals exposed this as
  necessary for safe same-machine agent coordination. The corrected rehearsal
  preserved Alpha's principal decline, gave Beta an independent unreviewed
  review request, kept work non-actionable until Beta continued, routed Beta's
  explanation back to Alpha without reopening Beta, and suppressed committed
  terminal attention across restart.

- **Mixed-surface handoff remains ordinary conversation.** A flat thread keeps
  one human-readable exchange while `in_reply_to_event_id` identifies the exact
  message answered. Source-naive Human→Alpha→Beta rehearsal, hard-crash
  redelivery, exact handled-result binding, and human projection now pass.

- **Private coordination preserves the ordinary confidentiality boundary.**
  Origin provenance and address hints stay inside the encrypted semantic event;
  excluded peers learn neither private room nor hint metadata. Decryption still
  does not replace semantic admission.

- **Beta evidence is causal and release-bound.** Local tests and loopback
  rehearsals cannot prove packaged Windows launch, physical non-loopback IPv6,
  actual assistive-technology/media operation, offline key custody, or public
  artifact readback. `docs/BETA_EVIDENCE.md` and the authenticated release
  manifest define those external gates, and beta.3 predates this branch.

## Current orientation

The local performance and consistency work is progress, but native evidence is
still behind main. Native access has returned; protected communicating apps remain
unchanged and stable development signing/Keychain authorization remain unverified.
Read THIMBLE-UX011-BOUNDARY-1 in the GUI and sent consolidated checkpoint
CODEX-ec1bc77-READY-1, superseding the unsent UX012–024 drafts. It is visible in
local room history; remote receipt remains unverified. GUI diagnosis fails for
Thimble's stored [2601:205:4b04:6520:ec4:7aff:fee6:b402]:45152 endpoint and sync
reports both known peers failed. Recover peer reachability without blind resend,
and resume disposable native rehearsals before changing the protected build.
Do not treat local private-history timings as completing onboarding, network
healing, cross-platform durability, or easy/predictable native UX.

UX-023 supplies Retry-After: 1 on actual daemon capacity-rejection responses and
tests saturated handlers/recovery. Thimble's HTTP 500 cause remains unresolved.
UX-024 rules out initialized-store schema setup as a writer-contention cause in
one local case: the bundled SQLite store opens and reads committed facts while
another connection holds BEGIN IMMEDIATE with an uncommitted write. The reader
excludes that write until commit. Preserve this regression and seek actual error
body/log evidence for the HTTP 500, rather than changing initialization on that
unproven hypothesis. This does not rule out filesystem failures, corrupt homes,
other contention paths, or Linux-specific behavior.
Further private-send reuse needs a proven freshness boundary; repeated timing
fixtures and allocation cleanups alone do not close that UX gap.

## Native catch-up findings

Current e7917b3 native host was rebuilt into the disposable UX Test app, whose
Info.plist points to ux-001-test-home with test-file vault. GUI public send/reopen
passes. GUI created private native-private-check with only the local member,
sent PRIVATE-NATIVE-e7917b3-1, rotated to epoch2 through the confirmation flow,
sent PRIVATE-NATIVE-e7917b3-2, and reopened. Both messages remain readable after
reselecting the private channel. This does not establish excluded-peer delivery,
production Keychain behavior, or multi-member native recovery.

New observed UX friction: restart returned to #general instead of preserving the
last selected private channel. Next fix should restore a valid accessible last
selection per home, with safe fallback when unavailable, without restoring stale
membership authority or mixing profile state. Protected communicating apps remain
unchanged; Thimble checkpoint remote receipt remains unverified.

## Prediction errors

- Native use exposed multi-second send/refresh stalls and Linux HTTP 429 under
  stale peer endpoints. Ordinary snapshot GET initiated sync under
  the shared command gate. Explicit peer sync now releases ordinary command serialization while a
  home-transition gate protects its store/identity lifetime (UX-009). Join and
  recovery still await network completion under shared command serialization.
  UX-014 measures retained public history: 1000-message local debug snapshots
  improve from ~328ms to ~221ms by reusing snapshot inputs and indexing profile
  updates. Private-history/50-member/Linux performance still needs measurement.
  UX-016 found a severe private-history regression: 100 private messages took
  ~11s per snapshot. Skipping irrelevant-room crypto in governance derivation
  and reusing one imported key bundle per reconstruction reduces this to ~787ms.
  UX-017 reuses creation-time governance within one core-owned semantic replay,
  with full derivation for incomplete dependencies and boundary changes. The same
  100-message private snapshot now averages ~147ms; 176 workspace tests, strict
  Clippy, and the native build pass. Larger/multi-member histories and native/Linux
  evidence remain pending.
  UX-018's first 1000-message private probe averages ~603ms per snapshot and
  takes 122s including fixture creation/measurement. The instrumented rerun
  verifies all 1000 texts after reopening: snapshot ~622ms, sends median115ms,
  p95 212ms, final223ms; history creation118s. The read path reconstructs
  each accessible room separately for unread counts, notifications, and the
  coordination frontier, plus selected timeline reconstruction. The send path
  reconstructs prior private meaning for semantic validation on every send.
  Next work should measure these separately and consolidate per-room projection
  inputs without retaining all decrypted histories across operations or weakening
  creation-time validation. The smaller fixture alone does not prove scaling.
  UX-019 reuses the selected room's validated history from unread counting for
  timeline/call, notifications, and coordination within the current snapshot.
  Other rooms remain individually reconstructed; no decrypted cache survives a
  refresh. Independent projection comparisons cover private/default/missing-room
  selections; 176 workspace tests, strict Clippy, and native build pass. The
  1000-message private probe improves snapshot622ms ->208ms and verifies all
  retained text after reopening. Sends remain median116ms/p95 214ms; private-send
  reconstruction and many-room scaling remain separate outstanding work.
  UX-020 identified a measurement gap: prior send timings covered low-level
  event creation, excluding the command host's retry-token lookup and response
  snapshot. The probe now measures the complete local command and three retries,
  verifies their shared event ID, and checks exactly one new message after reopen.
  Retry lookup itself reconstructs private history before the send path does it
  again. These local command timings still exclude native bridge/rendering and
  remote delivery, which require their own evidence. At private1000, the full
  command takes586ms; retries354–356ms return the same event and exactly one new
  retained message. These timings supersede low-level send times for claims
  about local command responsiveness.
  UX-021 removes duplicate-token comparison allocations and adds optional
  token-bearing fixture history (VOXELLE_PROFILE_REQUEST_IDS=1). Private100
  command148/151ms and snapshot86/84ms before/after do not establish a speedup;
  retained-text/retry checks and 176 workspace tests pass. Full-command history
  reuse needs a freshness guarantee for background admission, not an assumption
  that the first lookup remains current. Larger token-bearing histories remain
  an evaluation gap.
  UX-022 fixes mixed governance/room reads during background admission: one short
  SQLite read transaction now captures both retained inputs before decryption.
  A two-connection interleaving test fails without the boundary and passes with
  it; subsequent reads see newly committed facts and error paths release it.
  This is not yet a freshness mechanism for reuse across send/admission stages,
  and does not make every snapshot/key-import read atomic.
  Local acceptance must stay responsive independently of unreachable peers,
  while durable propagation and truthful peer-relative evidence remain intact.
- Thimble measured ~3-second local reads before stale peer import on Linux
  `61b98ee`, with three of six paired reads returning HTTP 429. Removing network
  waits alone does not prove responsive local projection. The continuation-frontier
  test also crossed a 60-second lease when run alone. UX-010 removes that
  machine-speed assumption: retained events are projected immediately before
  and exactly at expiry across selection and restart, checking active/overdue
  semantics and the next projection deadline. Thimble confirmed all three frontier tests on Linux f227bdc. Result sends returned HTTP 500 without verified
  delivery; retain uncertain-delivery handling while obtaining the concrete cause.
- Linux `14c8ebe` public-workflow sync closed and revoked-invite preflight
  occasionally succeeded (focused rerun passed). Address-monitor cancellation of
  active requests is now independently reproduced and fixed; whether it explains
  those Linux failures remains unverified. Preflight tolerates unavailable peers,
  so transport success/failure and authoritative revocation evidence must stay
  distinct during diagnosis.
- One concurrent test run could not rebind a saved listener port after stop;
  isolated and full reruns passed. Port reuse versus incomplete socket teardown
  remains unresolved; passing reruns do not prove stable restart reliability.
- Native connection setup required relaying raw certificates and manually
  replacing a changed port. A globally addressed runtime and an "Online"
  label did not establish peer connectivity. Profile names were known but
  unlabeled connection controls used opaque peer IDs. UX-013 now reuses shared
  profile names for these controls and health errors while retaining explicit
  local labels; native visual confirmation remains pending. UX-011 proves automatic
  bidirectional catch-up after one peer changes port while the other retains its
  stale record, provided a surviving outbound route exists. UX-012 adds device-signed, bounded, expiring listener exchange through current
  admitted space/device authority. A three-peer real-QUIC test learns the changed
  address through Carol, then delivers directly after Carol stops. Incoming
  ephemeral ports are not listener evidence. Native rehearsal is pending because
  Computer Use reports the Mac locked; Linux/non-loopback confirmation is pending.
- Rebuilding an ad-hoc macOS artifact changes Keychain identity. Disposable
  test homes can use the documented debug-only test vault; continuing homes
  must retain production protection and an explicit replacement/recovery path.
