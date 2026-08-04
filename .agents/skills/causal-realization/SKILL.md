---
name: causal-realization
description: Make a claimed software capability exist as a complete, real, evidence-backed causal path from actual input through authoritative decisions and state changes to observable output. Use independently for vertical slices, walking skeletons, risky substrate bring-up, end-to-end feature completion, interactive or distributed workflows, runtime verification, or deciding whether a feature is genuinely done rather than merely scaffolded, mocked, locally correct, or accepted by one layer.
---

# Causal Realization

A capability exists only when its real causes produce its claimed effects.

Construct and verify the whole path:

```text
real input
-> interpretation
-> authoritative decision
-> state transition
-> retention when required
-> propagation
-> projection
-> observable result
```

Do not confuse types, endpoints, state, tests, mocks, polished pixels, or a
successful intermediate handoff with a realized capability.

## Establish the realization contract

State before construction:

- **Capability:** externally meaningful behavior that must become true.
- **Envelope:** users, trust, scale, data, durability, latency, platforms,
  deployment, accessibility, integrations, and failure conditions for which it
  must be true.
- **Accepted revisions:** qualities that may differ, with explicit tolerances.
- **Exclusions:** adjacent behavior not being claimed.
- **Evidence horizon:** which runtimes, participants, inputs, failures, and
  artifacts can actually be inspected.
- **Risk frontier:** the least-proven substrate, process, persistence,
  integration, concurrency, or interaction boundary on which realization
  depends.

Write the central truth in one or two causal sentences:

> Clients propose messages; one authority orders and retains accepted messages;
> clients project those accepted facts.

Separate three claims:

- **Semantic:** the meaningful domain transition occurs.
- **Operational:** it survives the claimed retry, failure, restart, concurrency,
  and deployment conditions.
- **Lived:** a real user or consumer can perceive and control it with the claimed
  interaction, responsiveness, and presentation quality.

The contract may claim only the dimensions relevant to the capability, but
never use evidence for one as proof of another.

## Run the realization loop

### 1. Trace the causal loops

Trace representative behavior end to end. Include error, retry, disconnection,
restart, authorization, and recovery paths belonging to the envelope.

Interactive and asynchronous systems usually contain several loops. Trace them
separately—for example local editing, remote acceptance, replication, scrolling,
notification, reconnect, and animation—then identify where they meet and which
authority owns each meeting point.

For every asynchronous edge, name:

- the thread, event loop, timer, callback, or job that initiates it;
- the authority allowed to mutate the affected state;
- the thread or loop allowed to publish, render, or observe the result;
- the handoff and publication mechanism;
- what prevents overlap, stale publication, reordering, or duplicate work.

Produce a testable statement:

> Input X should cause decision A, transition B, retained fact C, propagation D,
> and observable result Y within envelope E.

### 2. Cross the risk frontier first

Construct the smallest executable vertical slice that crosses the least-proven
boundary and carries one real fact through the intended final shape.

If the system depends on a native callback, process boundary, durable append,
remote API, generated artifact, device, unfamiliar runtime, or human-visible
projection, prove that seam before surrounding it with broad feature work.

Prefer a real walking skeleton to a horizontal scaffold. Do not mock the risk
frontier unless the unavailable system is explicitly outside the evidence
horizon. Make the slice produce an inspectable output and fail clearly before
other machinery can obscure the cause.

### 3. Make the path whole

Implement the real input, decision, transition, retention, propagation,
projection, output, and required recovery behavior. Use domain vocabulary that
exposes causality and authority.

Keep input meaning and rendered geometry joined when practical. If pixels define
an interactive region, derive hit testing, focus, accessibility, or invalidation
from the same layout authority rather than maintaining a parallel spatial story.

Treat compiler failures, runtime exceptions, malformed artifacts, stale pixels,
missing callbacks, duplicate work, and failed recovery as causal evidence. A
failure at an ABI, event loop, persistence, network, rendering, or packaging seam
often means the assumed path or boundary does not actually exist. Repair the
causal map before adding compensating layers.

### 4. Verify the same causal truth

Match evidence to the contract:

- focused tests for decisions, transitions, ordering, idempotency, and contracts;
- clean builds when types, registrations, generated inputs, or dependencies
  change;
- artifact and data inspection for retained and transmitted meaning;
- real process, integration, authorization, restart, failure, and recovery probes
  when claimed;
- real input-to-output runtime workflows for lived behavior.

For a user-facing capability:

- launch the real packaged or deployed artifact;
- use the actual input classes in scope—keyboard, pointer, wheel, touch, device,
  timer, or remote arrival;
- inspect the pixels or other human-visible output rather than inferring quality
  from state;
- exercise relevant empty, populated, loading, disconnected, error, and recovery
  states;
- inspect hierarchy, density, contrast, alignment, feedback, motion, and
  responsiveness when those qualities are claimed;
- use multiple real participants or processes when realtime coordination is
  claimed.

Do not call a workflow verified merely because a proposal left the client or an
authority accepted it. Observe the accepted fact return through the real
projection path.

If verification fails, stop extending the feature. Classify the failure as a
wrong contract, missing causal edge, absent authority, substrate mismatch,
concurrency defect, or implementation defect. Repair the protected path and
rerun the same probe.

### 5. Remap and expand

After the first realized slice and each meaningful feature family, redraw the
causal loops. New behavior must either extend an existing story or introduce a
newly named truth with a complete path.

Remap after adding a process, persistence, trust, deployment, scheduling,
replication, caching, or invalidation boundary, and after the first runtime
failure at a new substrate.

Keep the next risk frontier explicit. Let each proven slice determine the next
construction step.

## Realization fixed point

Treat the scoped capability as realized only when:

1. Every claimed semantic effect has a real input-to-output path.
2. Every operational condition in the envelope has proportional evidence.
3. Every lived claim has been exercised through its actual user or consumer
   surface.
4. The highest-risk substrate and coordination paths were run, not merely
   compiled or mocked.
5. Remaining gaps are labeled as exclusions, accepted revisions, or unresolved
   claims rather than hidden behind a general statement of completion.

This is a local conclusion within the evidence horizon, not proof of every
possible runtime condition.

## Relation to compression

Causal realization may require adding machinery because an unrealized edge is
not removable complexity. It does not decide whether the resulting authorities
and coordination are topologically minimal or whether the embodiment is
physically fit.

Use topology compression to minimize the coordination needed to understand and
change the realized capability. Use embodiment compression to minimize the
physical machinery needed to execute it. Reverify causal realization after
either changes the path.

## Failure modes

| Failure | Correction |
|---|---|
| A scaffold or mock is called a feature | Carry a real fact through the production-shaped path |
| Server acceptance is called completion | Observe the accepted fact at the real consumer surface |
| State correctness substitutes for UI quality | Exercise actual inputs and inspect actual output |
| Familiar scaffolding postpones the risky substrate | Cross the risk frontier in the first slice |
| Several event loops mutate one story implicitly | Name mutation, publication, and scheduling authority |
| Happy-path tests substitute for the envelope | Probe claimed retry, restart, failure, and recovery behavior |
| A runtime failure gets wrapped in another layer | Correct the causal map and repair the broken seam |
| Completion exceeds available evidence | State the evidence horizon and unresolved claims |

## Realization report

```markdown
## Contract
[Capability, envelope, accepted revisions, exclusions, evidence horizon, and
semantic/operational/lived claims]

## Causal map
[Central truth, representative loops, authorities, schedulers, and risk frontier]

## Realized slice
[The real input-to-output path and the substrate boundary it proves]

## Evidence
[Tests, clean builds, runtime probes, artifacts, real inputs and outputs,
failures, recovery, and gaps]

## Result
[What is genuinely realized, bounded claims, and the next risk frontier]
```
