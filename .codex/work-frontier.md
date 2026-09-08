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
