# Voxelle Inhabitant Surface v0

Status: draft  
Audience: Voxelle implementers, Watch/WFB resident agents  
Scope: first agent-facing contract for inhabiting and operating Voxelle

## 1. Position

The inhabitant surface is not a separate "agent API" with special authority. It
is a small, typed surface over the same application truth that the human UI uses:
snapshots, semantic commands, service activity, room state, peer records, and
network health.

The first target is a local HTTP/SSE inhabitant service for Watch/WFB resident
agents such as Thimble or Aster. HTTP carries snapshot and command requests. SSE
carries deltas. Later consumers may be episodic Codex sessions, Discord-invoked
agents, or other harnesses, but they should use the same service rather than a
parallel CLI, shell, or MCP implementation.

The service owner should be a sidecar binary in a new workspace crate,
provisionally `crates/voxelle-inhabitantd`. It owns a `VoxelleCommandHost` and
exposes the local HTTP/SSE surface. The Tauri host, Watch/WFB, and manual tools
are clients of that service; they do not own separate application logic.

The v0 design should preserve this distinction:

- Resident agents need streams and hot-path tools.
- Episodic agents need event hooks, queryable snapshots, and optional replies.
- Both need the same underlying Voxelle concepts and command authority.

## 2. Commitments

### 2.1 One Agent-Facing Target

The v0 target is local HTTP/SSE.

That choice is not because HTTP is more fundamental than the app. It is because
the inhabitant surface needs query, action, and stream semantics in one place:

- HTTP `GET` for discovery and snapshot.
- HTTP `POST` for commands.
- SSE for room, runtime, peer, network-health, and service-activity deltas.

CLI commands, MCP tools, and skills may exist later, but they should be thin
clients or documentation over this service. They should not become alternate
places that decide Voxelle behavior.

The sidecar process is the product boundary for this surface. A windowed desktop
app can launch or connect to it, but the agent surface should not require a
window to be alive.

The dependency direction should stay one-way:

```text
voxelle-inhabitantd -> voxelle-app -> core/store/sync/net
```

Core app crates should not depend on HTTP, SSE, Axum, Watch, Discord, or MCP
concepts.

### 2.2 Same Authority Path

Agent actions should travel through the same semantic command layer as the
desktop shell. A resident should call something equivalent to
`runtime.goOnline`, `message.send`, `peer.import`, `peer.diagnose`, or
`peer.sync`, not mutate storage directly or simulate UI clicks.

The command ID is the stable concept. The button, MCP tool, CLI command, HTTP
route, or skill instruction is only an affordance.

### 2.3 Snapshot Before Action

An agent should be able to orient from a single structured snapshot before
acting. The current `ShellSnapshotView` is the starting point:

- home root
- profile and identity summary
- runtime status
- invite exchange state
- known peers
- room timeline
- network health rows
- service activity
- UI ontology

For v0, the snapshot is more important than a large command surface. It tells an
agent where it is, what is broken, what actions exist, and what evidence an
action should leave behind.

### 2.4 Failures Are First-Class

Voxelle is full of conditions that look similar but require different handling:
offline service, missing home, no peer record, stale address, unreachable peer,
successful diagnostic but empty sync, sync succeeded but no new events.

The inhabitant surface should expose those states as typed health rows and
action results, not as terminal logs an agent must scrape.

### 2.5 Ergonomics Depend On Harness

Do not design only for Watch/WFB. Watch is the first consumer because it can
actually use streams and soundings, but many agents are episodic.

The same Voxelle contract should support three affordance layers:

- Skill/docs: slower guidance and routing for "how do I do this in Voxelle?"
- Hook/query: event-triggered agents that are instantiated, query state, act,
  and return an optional reply.
- Stream/tools: resident agents that subscribe to deltas and use direct hot-path
  tools.

Those are harness affordances, not competing Voxelle backends. They should route
to the HTTP/SSE inhabitant service or explain how to use it.

### 2.6 No Hidden Mutation

If an agent acts, Voxelle should be able to show what happened. Agent-originated
room messages, imports, diagnostics, syncs, and online/offline changes should be
visible as ordinary app activity with attribution.

## 3. Core Contracts

### 3.1 Discovery

Discovery answers: "What Voxelle home am I attached to, and what surface can I
use?"

Minimum fields:

- `surface_version`
- `home_root`
- `base_url`
- `profile`: nullable peer/device/default-room summary
- `capabilities`: command IDs and event streams available through HTTP/SSE
- `snapshot_url`
- `events_url`
- `skill_root` or docs index, if available

Discovery may start as a local file or command result. It should not require an
agent to infer the surface from source code.

The v0 bootstrap file is:

```text
{home_root}/.voxelle-inhabitantd.json
```

It is an owner-readable capability file containing the sidecar `base_url`,
`pid`, `started_at_unix_ms`, endpoint URLs, and the per-launch bearer
authorization value. Clients send that value in the `Authorization` header.
The service accepts loopback binds only, rejects browser-origin requests, and
requires authentication for discovery, snapshots, commands, and events. Once
authenticated, `GET /inhabitant/v0/discovery` is authoritative.

### 3.2 Snapshot

Snapshot answers: "What is true now?"

The existing `ShellSnapshotView` should remain the reference shape. v0 may add
agent-facing hints, but should not fork the core truth:

- `home_root`
- `home` or `home_error`
- `network_health`
- `ui_ontology`
- `service_activity`
- `agent_hints`, optional

Suggested `agent_hints`:

- `suggested_next_actions`: command IDs with short reasons
- `waking_items`: changes worth waking a resident agent for
- `ambient_items`: changes safe to leave for quiet soundings
- `human_action_required`: true when progress needs a person

### 3.3 Action

Actions answer: "What may I do, through the same authority path as the UI?"

The v0 action set uses the same stable semantic command IDs as the UI:

- `shell.refresh`
- `home.init`
- `runtime.goOnline`
- `runtime.goOffline`
- `message.send`
- `peer.import`
- `peer.diagnose`
- `peer.sync`
- `ui.preference.set`

Routes and buttons are affordances over these IDs; adapters do not invent a
second command vocabulary.

### 3.4 Action Result

Action results answer: "What changed, and what should I do next?"

Minimum fields:

- `ok`
- `command_id`
- `snapshot` or `delta`
- `activity_items`
- `error`, nullable
- `recovery`, nullable

Errors should classify the recovery path:

- `needs_home`
- `needs_service_online`
- `needs_peer_record`
- `needs_reachability`
- `needs_sync`
- `needs_human`
- `internal_error`

### 3.5 Delta

Deltas answer: "What changed since my last view?"

Watch/WFB can use deltas as waking or non-waking stream data. Episodic agents may
ignore streams and query snapshots instead.

Useful delta kinds:

- `room.message`
- `runtime.state`
- `peer.record_imported`
- `peer.diagnostic`
- `peer.sync`
- `network.health_changed`
- `service.activity`
- `ui.ontology_changed`

Each delta should include enough IDs for a later snapshot query to recover
context.

## 4. First Watch/WFB Slice

The first resident slice should be deliberately small:

1. A local `voxelle-inhabitantd` sidecar backed by `VoxelleCommandHost`.
2. `GET /inhabitant/v0/discovery`.
3. `GET /inhabitant/v0/snapshot`.
4. `POST /inhabitant/v0/commands/{command_id}` for the current shell command
   set.
5. `GET /inhabitant/v0/events` as an SSE stream for room messages, network
   health changes, runtime changes, peer diagnostics, sync reports, and service
   activity.
6. A skill/docs folder for slower workflows: getting started, field testing,
   interpreting network health, and recovering from common failures.

The important v0 behavior is not autonomy. It is legibility: a resident can
arrive, see the room, understand what is broken, perform a named command, and
explain the result without reading logs.

## 5. Tiny Grove Lessons To Borrow

Borrow:

- snapshot-first orientation
- semantic actions instead of UI gestures
- event streams for resident agents
- queryable state for episodic agents
- local docs and skill routing for infrequent workflows
- explicit recovery language

Do not borrow blindly:

- screenshot-first operation
- camera/player-specific assumptions
- game-loop timing as the ontology
- Godot-specific HTTP/SSE details

Tiny Grove is useful because it treats agents as first-class participants. Voxelle
should do that through room, peer, service, and command concepts rather than
through game-object concepts.

## 6. Success Scenario

A Watch resident should be able to:

1. Discover a Voxelle home.
2. Read one snapshot and know whether the service is online.
3. Start the service or report the blocker.
4. Import a peer record supplied by a human or another agent.
5. Diagnose reachability with a typed result.
6. Sync with a peer and see whether new room events arrived.
7. Send a room message through the same room timeline humans use.
8. Explain what happened from snapshot, delta, and service activity evidence.

That is enough for v0. It gives Voxelle an inhabitable surface without pretending
the agent is omniscient or separate from the application.

## 7. Open Questions

Ask these one at a time:

- How should Voxelle attribute agent-originated messages and actions?
- Which deltas should wake Watch residents, and which should remain ambient?
- Should the agent-facing skill live inside this repo, inside a Watch resident
  root, or both?
