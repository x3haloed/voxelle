# Reticulum as Secondary Transport: UX Vision (Draft)

Status: draft design proposal for branch `codex/reticulum-secondary-transport`

## 1) Scope and preserved invariants

Reticulum is planned as an **optional secondary transport path** only.

- Preserved: protocol truth still flows through existing signed room governance and acceptance.
- Preserved: transport remains a bearer of already-authenticated facts (no route-level authority).
- Preserved: user-visible membership/governance/recovery workflow remains as-is.

References

- `docs/TRUTHFUL_SYSTEM_CONTRACT.md` (directed governance and evidence-first operation)
- `docs/P2P_INVITE_SPACE_CHAT_RFC.md` (untrusted provider/relay as availability surface)
- `docs/IPV6_NATIVE_P2P_SPEC.md` (preferred QUIC transport, transport role boundaries)

## 2) UX objective

Make secondary transport behavior mostly automatic, with only targeted explanatory surfaces when QUIC paths degrade.

- Keep normal actions unchanged: start with existing **runtime** and **peer** commands.
- Add clarity around *what transport path succeeded* and *why fallback happened*.
- Preserve stable semantics for existing command IDs:
  - `runtime.goOnline`
  - `runtime.goOffline`
  - `peer.diagnose`
  - `peer.sync`
  - `invite.copy`

## 3) Proposed UI surfaces (low-disturbance plan)

### 3.1 Status place (`runtime.status` + `network.health`)

Add two new `NetworkHealth` rows under existing `Network Health` behavior:

1. `transport` (label: **Transport**)
- Working: transport fabric healthy (`quic+reticulum`) with active session summary.
- Working: `quic only` when only preferred transport is active.
- Needs attention: all outbound/inbound attempts are degraded and retrying via alternate path.

2. `fallback` (label: **Fallback**)
- Working: no fallback in use.
- Needs attention: secondary transport in use for at least one peer.
- Broken: all secondary paths unavailable and no stable QUIC route.

Both rows should keep existing behavior:
- `related_view("runtime.status")`
- `related_view("network.health")`
- `related_command("runtime.goOnline")` for actionable remediation.

### 3.2 Service Activity stream

Tag sync/diagnostic activity items with transport metadata, examples:

- `peer.sync` event: `path=quic` / `path=reticulum`
- `peer.diagnose` result: transport-specific failure reason buckets.
- In Voxelle surface text currently implemented as `via path=<transport>` tags (for now `path=quic`).

This is visible for debugging but should not dominate normal timelines.

### 3.3 Message delivery affordance

In message or room failure surfaces, use one short typed hint:

- “Sent via fallback transport; retry when local route is restored.”

Keep as plain, non-alarming language.

### 3.4 Advanced setting (opt-in)

Under settings/advanced only:

- `network.transport.fallback.enabled`
- `network.transport.fallback.display_name`
- `network.transport.fallback.endpoint`
- Optional explicit path exclusion list (for debugging only)

No new first-class public toggle in defaults.

## 4) Behavior model by scenario

### 4.1 Ordinary success

- QUIC route works end-to-end.
- Row status:
  - `transport`: Working (`quic`)
  - `fallback`: Working (`unused`)
- No extra user action required.

### 4.2 QUIC unstable, Reticulum used

- Automatic failover to Reticulum as a secondary path.
- Row status:
  - `transport`: Needs attention (primary degraded)
  - `fallback`: Working (`reticulum active`)
- Activity detail includes endpoint class + peer count + reason (`connect/relay/ttl` etc.).

### 4.3 Fallback unavailable

- If both routes fail:
  - `transport`: Broken (`no validated route`)
  - `fallback`: Broken (`no forwarding path available`)
- Related command remains `runtime.goOnline` and `peer.diagnose` for next action.

## 5) Disturbance estimate by scope

1. **Recommended path (recommended): low-to-medium**
- Changes stay in `network.health`, `service.activity`, and one-line diagnostics.
- No new command-level workflows.

2. **Moderate path (if needed): medium**
- Add settings view and path selector.
- Slightly more visible operational UI in `workbench/preferences`.

3. **High path (not recommended for first pass): high**
- Explicit transport picker during every `runtime.goOnline`/peer action.

## 6) Non-goals (important)

- Do not imply Reticulum as trusted infrastructure.
- Do not make invite flow or bootstrap interpretation depend on Reticulum.
- Do not add new authority-bearing UI controls for transport policy.
- Do not alter accepted facts, membership logic, or replay semantics.

## 7) Implementation checklist (minimum slice)

1. Add network-health rows and copy text for `quic` / `reticulum` state.
2. Extend service-activity payload to include transport path metadata.
3. Add short user-facing degraded-path hint in room/ping failure state.
4. Add optional advanced preference guardrails and persist defaults.
5. Add field-test appendix item:
- peer joins and converges through non-QUIC path while inviter remains offline.
- visible failure state maps to exact remediation command.

Note: the implemented fallback path currently uses an explicit IPv6 endpoint preference as the secondary dial target and keeps Reticulum naming stable for path visibility.

## 7a) Completed in this branch (minimum slice)

- Added `transport` and `fallback` network-health rows:
  - `transport` reports primary transport visibility and whether the service is online.
  - `fallback` reports preference state; active secondary-path status is not yet surfaced in local telemetry.
- Updated peer diagnostic/sync activity summaries to emit transport-label metadata.
- Updated service event summaries to emit transport path text for diagnostics and sync.

## 8) Invariant checkpoints

- `UI ontology` view/command IDs unchanged.
- One semantic command path remains authoritative.
- Transport choice does not alter admission, revocation, or recovery truth.
