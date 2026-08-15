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
- `coordination_snapshot_url`: the compact snapshot intended for repeated
  resident-agent reconciliation
- `events_url`
- `commands_url`: the route template for semantic commands
- `contract_url`
- `command_transport`: the HTTP method, content type, bearer-header shape, and
  request/response envelope
- `command_semantics`: machine-readable retry and observation guidance for
  commands whose success could otherwise be over-interpreted
- `replay_policy`: explicitly `none`; reconnecting consumers reconcile the
  coordination snapshot rather than requesting missed stream events
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
- `home` or structured `home_error` (`message`, `recovery_message`, `detail`,
  and Rust-owned `recovery` category); a genuinely fresh home has neither
- `network_health`
- `ui_ontology`
- `service_activity`
- `sync_evidence`: a peer-relative result (`unknown`, `peer_confirmed`,
  `partial`, or `unreachable`) with attempted/reached peers and event counts;
  it never means globally current
- `agent_hints`, optional

Suggested `agent_hints`:

- `suggested_next_actions`: command IDs with short reasons
- `waking_items`: changes worth waking a resident agent for
- `ambient_items`: changes safe to leave for quiet soundings
- `human_action_required`: true when progress needs a person

Network-health rows may carry `primary_action_payload` together with their
stable `primary_action`. This lets both human and agent affordances retry the
exact failed ordinary peer without selecting a different endpoint or parsing
human prose. Peer-operation failures are availability observations, never
membership or governance authority.

### 3.3 Action

Actions answer: "What may I do, through the same authority path as the UI?"

The v0 action set uses the same stable semantic command IDs as the UI:

- `shell.refresh`
- `home.init`
- `home.archiveForRecovery`
- `runtime.goOnline`
- `runtime.goOffline`
- `message.send`
- `message.acknowledge`
- `message.continuation.update`
- `peer.import`
- `peer.diagnose`
- `peer.sync`
- `ui.preference.set`

Routes and buttons are affordances over these IDs; adapters do not invent a
second command vocabulary.

Every shell-scoped command in the snapshot's shared `ui_ontology.commands`
also names its Rust request DTO in `payload_type`, or carries `null` when its
payload is empty. Discovery exposes an authenticated `contract_url` serving the
generated TypeScript declarations for those DTOs. The checked-in WebView
contract and the served agent contract are generated from the same Rust types,
and a completeness test refuses a command whose named request type is absent.
These declarations make payload construction legible; authoritative bounds and
permissions are still enforced only when the semantic command reaches Rust.

`message.send` accepts a caller-generated `client_request_id`. Its retry scope
is the same principal, authorizing device, room, and semantic payload. Reusing
the ID for the identical request returns the originally admitted message;
reusing it for a conflicting payload is rejected. The ID is projected on the
message so a caller can reconcile a lost HTTP response after restart.

`message.acknowledge` creates an ordinary signed, admitted room fact with the
state `observed` or `handled`. `handled` is monotonic for that participant and
message. It may name a `result_event_id`, but only for `handled` and only when
that event is the handler's visible, already-admitted ordinary message threaded
to the target in the same room. The message acknowledgement projection retains
the deterministic set of admitted result IDs; independently authorized devices
that concurrently name different results therefore expose `result_conflict`
instead of choosing by arrival time. A result binding is an assertion by the
participant—not proof of correctness—and
private-room acknowledgements follow the same encrypted room path as private
messages. Acknowledging also advances the acknowledging home's local read
cursor through the target message, but not through later events. A local
`channel.markRead` cursor is deliberately different: it is
not replicated and is not sender-visible. Selecting or opening a channel does
not silently advance that cursor at the semantic-command layer.

An automatic `runtime.goOnline` persists the successful concrete listen and
advertised sockets and reuses them after a clean service or process restart.
This keeps existing peer availability hints usable for ordinary continuation;
an explicit Bind or Advertise request replaces the saved automatic binding.
Failure to reclaim a saved socket remains explicit rather than silently moving
to a new endpoint and making other members' retained hints stale.

`message.continuation.update` publishes a separate ordinary room fact rather
than overloading durable observation. `continuing` requires a relative lease
from one minute through seven days; `released` and `declined` carry no lease.
Each update names the same participant's known continuation heads that it
supersedes. A single unexpired head projects Continuing. Expiry projects
Unknown with `overdue: true`; it never proves that the participant stopped.
Concurrent unsuperseded device updates project Conflict until a new update
supersedes every head. Runtime reachability and sync evidence remain separate.
Lease expiry is a time-derived projection, not a new retained fact, and emits
no stream event. Consumers schedule a local snapshot refresh at `expires_ms`
and refresh after reconnect rather than waiting for an event that cannot exist.
The coordination snapshot GET is observational and never initiates peer sync.
Its `current_sequence` covers admitted or invalidated state, while
`projected_at_ms` timestamps time-derived fields; heartbeat is not evidence of
a semantic transition. A handled acknowledgement is completion evidence and
does not retroactively rewrite a separate continuation assertion.

`home.coordination_frontier` is a bounded, rebuildable attention index over
ordinary admitted messages in every currently accessible room. It reports
literal mention, acknowledgement, handled-result, reply, and continuation
facts; it never infers assignment, work, success, failure, presence, stopping,
or abandonment. Room selection and human read state do not remove entries.
`matching_count`, `omitted_count`, and `truncated` make projection bounds
explicit, and `next_projection_change_ms` identifies the next continuation
expiry that requires a time-derived refresh. Private entries exist only after
ordinary membership, decryption, and semantic admission and are never stored
as a parallel plaintext index.
Each `target_summary` is only an orientation preview. Its
`target_summary_truncated` and `target_summary_original_chars` fields state
whether content was abbreviated; a consumer opens the ordinary target by room
and event ID before taking consequential action from an abbreviated preview.
Frontier-level truncation fields describe omitted rows only.

### 3.4 Durable resident observation

Resident observation is local resumption bookkeeping over the same admitted
conversation model. It is neither a replicated event nor an agent-only task
store. A caller opens a stable `consumer_id` with `from_beginning` or
`from_now`, then calls `resident.observation.page`. The first page omits
`fact_high_water` and `after_fact_sequence`; later pages repeat the returned
high water and exact next sequence while `has_more` is true. Roots and their
ordinary replies span the exact accessible room set captured by the first
page. The final page alone returns a one-use commit token.

`fact_high_water` and each thread's `last_fact_sequence` are durable,
home-local first-admission ordinals. They are not SSE `current_sequence`, event
order, wall-clock order, channel read state, acknowledgement, or protocol
authority. After process restart, the resident begins paging again with its
stable consumer ID; uncommitted work is safely returned again. This is
at-least-once delivery, so downstream actions remain idempotent.

`resident.observation.commit` requires the final token and matching high water
and advances only that consumer across the exact served room set. It never
marks a channel read, publishes an acknowledgement, changes continuation,
proves handling or correctness, synchronizes peers, or emits global
`snapshot.changed`. `resident.observation.release` explicitly deletes only
that local consumer and its progress. Consumer IDs are local namespaces, not
principals, devices, credentials, or actor identities.

Pages enumerate only currently accessible channels and carry private facts
only through ordinary decryption and semantic admission. Page progress and
commit tokens are process-local and intentionally disposable; committed
consumer progress and local fact ordinals are durable.

### 3.5 Action Result

Action results answer: "What changed, and what should I do next?"

Minimum fields:

- `ok`
- `command_id`
- `snapshot` or `delta`
- `activity_items`
- `error`, nullable
- `recovery`, nullable

An `ok` command result proves that the local authority path accepted the
command. It does not by itself prove remote propagation, observation, handling,
or correctness. Those later claims require, respectively, peer-relative
`sync_evidence` and admitted participant acknowledgements.

`activity_items` contains only retained service-activity rows whose monotonic
IDs were created during that serialized HTTP action. The sidecar serializes
snapshot refreshes and agent command requests around the Rust-owned activity
cursor so concurrent callers cannot claim one another's results. Successful
actions derive the rows
from their returned authoritative snapshot; failed actions may report activity
the Rust host recorded before returning its structured error. An empty list is
an honest result for actions that produce no service activity.

Errors should classify the recovery path:

- `needs_home`
- `needs_service_online`
- `needs_peer_record`
- `needs_reachability`
- `needs_sync`
- `needs_input`
- `needs_human`
- `internal_error`

The serialized `ShellError` owns that classification together with its human
message, recovery instruction, and technical detail. HTTP action results may
repeat the same recovery value for convenient routing, but adapters must not
infer it by parsing error prose or invent a second classification.
`needs_input` means the command reached the authoritative validator but one or
more supplied values violated a documented semantic bound; the caller should
correct the payload rather than retry it unchanged or report an internal fault.

### 3.6 Delta

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

The first implemented waking event is the deliberately non-semantic
`snapshot.changed` notice. It carries a process-monotonic `sequence`,
`at_unix_ms`, and the compact `coordination_snapshot_url`. A successful HTTP command emits
the notice after the Rust command host has returned its new snapshot; inbound
peer-service activity emits through the same host invalidation callback.
Subscribers then fetch the authoritative snapshot instead of asking the HTTP
adapter to reconstruct room, governance, or recovery meaning. A lagged
subscriber must re-read the coordination snapshot.

The stream does not promise replay. `service.ready` therefore includes the
current process sequence and an explicit instruction to fetch the coordination
snapshot before acting. On every connection or reconnection, a resident treats
SSE only as an invalidation hint and reconciles a snapshot; it must not infer
that silence means currency, delivery, observation, or completion. The
coordination snapshot omits the large product component and UI ontology while
retaining the shared conversation and authority projection.
The coordination snapshot carries its own `current_sequence`. The sidecar reads
the process sequence before and after projection and retries if it changed, so
a resident can fetch until the snapshot sequence reaches the value announced by
`service.ready` or `snapshot.changed`. A persistently changing projection may
return a conflict and should be retried; this is preferable to claiming a
revision the snapshot did not embody.
More specific delta kinds remain future projections over that same snapshot
authority, not separate admission paths.

## 4. First Watch/WFB Slice

The first resident slice should be deliberately small:

1. A local `voxelle-inhabitantd` sidecar backed by `VoxelleCommandHost`.
2. `GET /inhabitant/v0/discovery`.
3. `GET /inhabitant/v0/snapshot`.
4. `POST /inhabitant/v0/commands/{command_id}` for the current shell command
   set.
5. `GET /inhabitant/v0/events` as an SSE stream. The implemented v0 stream
   provides `service.ready`, monotonic `snapshot.changed` waking notices, and
   heartbeats; specific room, network-health, runtime, diagnostic, sync, and
   service-activity delta kinds remain to be derived without duplicating the
   snapshot's authority.
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
