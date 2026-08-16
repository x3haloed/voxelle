# Work Frontier

## Outcome

Voxelle is ready for a credible beta when people and agents can install or attach to the product, join the same spaces, and sustain understandable coordination through identity, invitation, communication, recovery, customization, and small-group media paths. Agents must be able to distinguish local acceptance, propagation, observation, response, interruption, and conversation continuity without a parallel product model. Beta claims remain bounded by the truthful system contract and supported by proportional native, multi-peer, platform, accessibility, agent, and release evidence.

## Goal invariants

- **Invariant:** Ordinary successful use presents human goals before topology, protocol, or workbench machinery; deeper controls remain discoverable when customization, degraded operation, or intervention requires them.
  **Evidence:** `docs/TRUTHFUL_SYSTEM_CONTRACT.md` (Trust And Authority, Progressive topology), `docs/UI_ONTOLOGY.md` (Customization Must Be In-App Reachable, Topology Should Be Progressive), and direct inspection of the first-run desktop surface.

- **Invariant:** Human and agent affordances must invoke the same stable semantic commands and Rust-owned authority path.
  **Evidence:** `docs/TRUTHFUL_SYSTEM_CONTRACT.md` (Topology Preservation), `docs/UI_ONTOLOGY.md` (Commands), and `docs/inhabitant-surface-v0.md` (Same Authority Path).

- **Invariant:** A space and conversation remain one interoperable product surface regardless of whether people, agents, or both began or continued them; agent coordination cannot depend on hidden agent-only rooms, identities, messages, or acceptance rules.
  **Evidence:** User-specified beta outcome and `docs/TRUTHFUL_SYSTEM_CONTRACT.md` (human/agent affordances and shared semantic command authority).

- **Invariant:** Replicated coordination facts retain authenticated provenance for the local interaction session that requested them without turning that session into a principal, member, role, permission subject, or claim of human/agent nature. Principal and authorized-device signatures remain authorship and protocol authority; origin provenance explains only “via which device-certified surface session.” Resident observation consumers remain separate local checkpoint namespaces, and private-room origin stays inside the encrypted semantic event.
  **Evidence:** Core/app/sidecar tests cover certificate validation, native WebView and inhabitant routes, hashed-secret restart behavior, the three fact-producing gates plus four observation-ownership gates, projection, and private outer-envelope omission. Source-blind rehearsals proved two distinct resident sessions on one principal/device, wrong-secret and missing-origin recovery, remote public/private propagation, exact raw assertion/head attribution, simultaneous restart, durable resident redelivery, and owner-bound collision/token/release behavior across restart. Raw private ciphertext remains intentionally unavailable through the inhabitant API, so its outer-envelope omission is storage inspection evidence rather than source-blind API evidence.

- **Invariant:** Each resident resumes its own unobserved actionable conversation changes after disconnect or restart without human or sibling-resident activity advancing that progress; this local checkpoint must remain distinct from human read state, signed acknowledgement, handling, and protocol authority.
  **Evidence:** A source-blind resident disconnected after explicitly reading root A; sibling activity added a reply, handled result, and two roots, then advanced the shared home read cursor to the newest root. Reconnect retained every fact but reported unread zero, forcing either an unbounded rescan or dropped work. The implemented correction durably binds each consumer, page session, and commit token to `owner_origin_id` derived from authenticated origin context—not JSON—and requires that origin for open/page/commit/release. A source-blind Alpha/Beta rehearsal proved foreign page/open/commit/release attempts were unavailable and non-mutating, Alpha's token remained valid, and ownership/cursor isolation survived restart.

- **Invariant:** An ordinary message may carry bounded signed origin-session routing hints without changing full-feed delivery or creating assignment, obligation, presence, visibility, membership, handling, or task authority. Only an owner-authenticated observation page derives `addressed_to_owner`; private hints remain inside the encrypted semantic event.
  **Evidence gap:** A source-blind sibling-resident delegation attempt could not structurally identify the intended local origin, leaving both feeds identical and requiring prose inference. Implementation tests and fresh public/private, spoof, restart, and full-feed source-blind evidence are pending.

- **Expected:** Offline invite creation identifies the recoverable runtime prerequisite.
  **Observed:** A source-blind restart probe returned `internal_error` with detail “go online before creating a space invite.” The classifier should return `needs_service_online` and direct `runtime.goOnline`; rebuilt-surface confirmation is pending.

- **Invariant:** Coordination state distinguishes a fact being accepted, propagated, observed, answered, or abandoned; silence and disconnection must not masquerade as successful continuation.
  **Evidence:** User-specified agent coordination outcome; current independent black-box experiments are evaluating the required embodiment.

- **Invariant:** Effective actionability is derived per participant and target from the causal maxima of handled acknowledgements and continuation heads. Raw admitted facts remain preserved; incomparable maxima expose conflict; a causally later continuing assertion can resume after handled, declined, or released. Replies remain actionable until explicitly bound as a handled result or causally covered by every maximal disposition. Timestamps and home-local admission ordinals never resolve semantic heads, and no projection becomes global task authority.
  **Evidence:** Projection permutation/skew/private-room tests plus three source-blind built-surface rehearsals. Continuing then handled became non-actionable without deleting raw facts; later continuing resumed; a bound result stayed covered; a later ordinary follow-up became actionable with its exact uncovered reply ID; durable redelivery and clean restart preserved the same decision. Concurrent causal maxima are covered by deterministic projection tests but remain an explicit black-box evidence gap.

- **Invariant:** An ordinary clean restart on the same device preserves its usable advertised endpoint or fails explicitly; durable identity and conversation state do not constitute continuity when every peer silently retains an unreachable address.
  **Evidence:** A source-blind two-agent run preserved handled/result facts but could not continue after an ephemeral-port restart; the follow-up run reclaimed the saved endpoint and continued through the original peer record.

- **Invariant:** A beta claim requires completed causal paths and proportional evidence from native and physical environments; preview fixtures and polished pixels are not substitutes.
  **Evidence:** `docs/TRUTHFUL_SYSTEM_CONTRACT.md` (Causal Claims, Evidence Horizon, Construction And Verification Order) and `docs/BETA_EVIDENCE.md`.

- **Invariant:** In-progress human input survives ordinary refreshes, focused reviews, and cancellation until the person explicitly saves, submits, clears, or abandons it.
  **Evidence:** A rendered Customize probe showed that opening and canceling the all-customization reset review re-rendered the utility and silently replaced an unsaved checkbox draft with its projected value.

## Prediction errors

- **Expected:** An origin-addressed reply gives a resident enough structure to return and bind a readable result to the exact delegated exchange.
  **Observed:** The first source-naive rehearsal could identify Alpha's reply as addressed to Beta, but the old single `thread_root_event_id` field forced Beta to choose between an unrepresentable nested reply and overclaiming the broader human root. The correction preserves the flat thread root while adding an exact causal `in_reply_to_event_id`; handled results bind to that exact target.
  **Evidence:** A rebuilt source-naive Human-to-Alpha-to-Beta rehearsal survived a hard pre-commit crash, reopened both origin and consumer, returned Beta's ordinary result under the human root while directly answering Alpha, bound handled to Alpha's exact delegation, projected the readable chain to the human surface, and drained Beta's durable cursor to empty.

- **Expected:** Two cleanly stopped online peers can restart together and immediately serve local coordination snapshots.
  **Observed:** A second full addressed-flow rehearsal reproduced the hang: `resident.origin.open` held the global command gate while obtaining the device ID through a full restarted-home snapshot. Origin authentication now reads the identity vault directly and no longer drains service events or projects the home while holding that gate.
  **Evidence:** A deterministic restart handler regression plus a rebuilt source-blind full addressed flow: origin reopens completed in 5--12 ms, coordination in 186 ms, consumer reopen in 9 ms, and the persisted empty cursor page in 75 ms.

- **Expected:** A resident can checkpoint its own processing independently of other surfaces sharing the same home.
  **Observed:** Durable consumer-scoped changed-thread pages now survive process restart, remain independent of sibling consumers and human read/selection/open activity, bind final commits to the exact served high water and room set, and emit no global wake. Three source-blind runs found no consequential failure.
  **Uncertain:** Whether pagination beyond the exercised two-page loop, private exclusion across a non-member ciphertext retainer, and rare multi-room partial storage failure need deeper embodiment before beta rather than bounded follow-up evidence.
  **Evidence:** Store/app tests plus source-blind FromBeginning/FromNow, two-consumer, crash-before-commit, response-loss, SSE-boundary, public/private multi-room, invalid-token, and restart experiments.

- **Expected:** An explicit decline can communicate enough context for a person or agent to choose the next useful action.
  **Observed:** Independent source-blind residents can distinguish absent assertion, bounded continuing intent, expired unknown/overdue intent, release, decline, and handled threaded results across sync and restart. A decline currently carries no structured reason, so its cause requires a separate ordinary reply whose association is conventional rather than explicit.
  **Uncertain:** Whether a bounded decline reason belongs on the assertion, should bind an ordinary threaded reply, or is unnecessary once lived mixed human/agent trials exercise the workflow.
  **Evidence:** Three source-blind continuation lifecycle, adversarial, and stream experiments against the built inhabitant service.
