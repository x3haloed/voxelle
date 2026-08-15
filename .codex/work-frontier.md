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

- **Invariant:** Coordination state distinguishes a fact being accepted, propagated, observed, answered, or abandoned; silence and disconnection must not masquerade as successful continuation.
  **Evidence:** User-specified agent coordination outcome; current independent black-box experiments are evaluating the required embodiment.

- **Invariant:** A beta claim requires completed causal paths and proportional evidence from native and physical environments; preview fixtures and polished pixels are not substitutes.
  **Evidence:** `docs/TRUTHFUL_SYSTEM_CONTRACT.md` (Causal Claims, Evidence Horizon, Construction And Verification Order) and `docs/BETA_EVIDENCE.md`.

- **Invariant:** In-progress human input survives ordinary refreshes, focused reviews, and cancellation until the person explicitly saves, submits, clears, or abandons it.
  **Evidence:** A rendered Customize probe showed that opening and canceling the all-customization reset review re-rendered the utility and silently replaced an unsaved checkbox draft with its projected value.

## Prediction errors

- **Expected:** A successfully handled delegated request also exposes its result without requiring prose interpretation.
  **Observed:** Independent source-blind agents can now distinguish durable local admission, peer-relative propagation, recipient observation, and recipient handling. Idempotent request IDs and signed acknowledgements survive restart, and reconnect performs bounded known-peer catch-up. However, `handled` does not bind a result event, and no presence or lease semantics distinguish a paused conversation from an abandoned one.
  **Uncertain:** Whether the minimal interoperable continuation model should bind acknowledgements to ordinary reply event IDs, add expiring participant/session presence, or both, without turning agent tasks into a hidden product beside human conversation.
  **Evidence:** Source-blind two-home daemon experiments exercised retry conflict, offline send, reconnect catch-up, observed and handled propagation, participant disconnection, and restart durability through only advertised discovery, contract, coordination snapshot, commands, and SSE.
