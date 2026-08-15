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

- **Invariant:** Each resident resumes its own unobserved actionable conversation changes after disconnect or restart without human or sibling-resident activity advancing that progress; this local checkpoint must remain distinct from human read state, signed acknowledgement, handling, and protocol authority.
  **Evidence:** A source-blind resident disconnected after explicitly reading root A; sibling activity added a reply, handled result, and two roots, then advanced the shared home read cursor to the newest root. Reconnect retained every fact but reported unread zero, forcing either an unbounded rescan or dropped work.

- **Invariant:** Coordination state distinguishes a fact being accepted, propagated, observed, answered, or abandoned; silence and disconnection must not masquerade as successful continuation.
  **Evidence:** User-specified agent coordination outcome; current independent black-box experiments are evaluating the required embodiment.

- **Invariant:** An ordinary clean restart on the same device preserves its usable advertised endpoint or fails explicitly; durable identity and conversation state do not constitute continuity when every peer silently retains an unreachable address.
  **Evidence:** A source-blind two-agent run preserved handled/result facts but could not continue after an ephemeral-port restart; the follow-up run reclaimed the saved endpoint and continued through the original peer record.

- **Invariant:** A beta claim requires completed causal paths and proportional evidence from native and physical environments; preview fixtures and polished pixels are not substitutes.
  **Evidence:** `docs/TRUTHFUL_SYSTEM_CONTRACT.md` (Causal Claims, Evidence Horizon, Construction And Verification Order) and `docs/BETA_EVIDENCE.md`.

- **Invariant:** In-progress human input survives ordinary refreshes, focused reviews, and cancellation until the person explicitly saves, submits, clears, or abandons it.
  **Evidence:** A rendered Customize probe showed that opening and canceling the all-customization reset review re-rendered the utility and silently replaced an unsaved checkbox draft with its projected value.

## Prediction errors

- **Expected:** A resident can checkpoint its own processing independently of other surfaces sharing the same home.
  **Observed:** Home read state is shared; other surface activity can advance it while a resident is disconnected. The process SSE sequence detects a gap but resets on restart and has no durable mapping to retained facts. Storage now assigns first-admission-only local fact ordinals and persists bounded independent consumer checkpoints, but no app/inhabitant projection or commit surface carries them yet.
  **Uncertain:** The smallest served changed-thread page, consumer open/commit/release flow, and high-water binding that provides honest at-least-once resumption without becoming protocol authority.
  **Evidence:** Source-blind reconnect experiment; source inspection of shared `rooms.read` and process-global SSE sequence; store tests for duplicate-stable durable ordinals and independent monotonic consumer checkpoints.

- **Expected:** An explicit decline can communicate enough context for a person or agent to choose the next useful action.
  **Observed:** Independent source-blind residents can distinguish absent assertion, bounded continuing intent, expired unknown/overdue intent, release, decline, and handled threaded results across sync and restart. A decline currently carries no structured reason, so its cause requires a separate ordinary reply whose association is conventional rather than explicit.
  **Uncertain:** Whether a bounded decline reason belongs on the assertion, should bind an ordinary threaded reply, or is unnecessary once lived mixed human/agent trials exercise the workflow.
  **Evidence:** Three source-blind continuation lifecycle, adversarial, and stream experiments against the built inhabitant service.
