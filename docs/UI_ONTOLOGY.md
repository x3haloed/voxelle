# Voxelle UI Ontology

Status: implemented registry baseline
Audience: implementers  
Scope: first-pass ontology for the customizable desktop UI

## 1. Position

Voxelle's user interface should expose the application's own decomposition.

Customization is not only theme support. Theme support answers, "Can a user change
the appearance if they learn the format?" Voxelle should aim for a stronger rule:

Every exposed primitive should eventually have an obvious editing surface inside
the application.

The goal is high expressiveness and high reachability:

- Expressiveness: users can meaningfully change the app.
- Reachability: users can discover and make one small change without becoming
  theme authors, reading documentation, or editing hidden configuration files.

This document names the first UI primitives Voxelle is allowed to have. It is not
a complete design system and not a visual design spec.

## 2. Core Commitments

### 2.1 The UI Is Ontological Before It Is Decorative

The UI should name the kinds of things that exist before it paints them.

This means code should have explicit concepts for places, views, commands,
semantic tokens, metrics, behaviors, renderers, and editing surfaces.

If a thing is user-visible and likely to become customizable, it should not be
buried as an incidental implementation detail.

### 2.2 Customization Must Be In-App Reachable

User-editable primitives should move toward in-app editing surfaces.

Raw files, JSON overrides, and exported theme packages may exist later, but they
are not the primary customization experience.

The reference path should be:

1. User sees something.
2. User can discover what kind of thing it is.
3. User can change the relevant primitive from the app.
4. The app persists that change as user preference state.

### 2.3 The Runtime Truth Stays in Rust

The UI shell may use web technology, but protocol, storage, sync, networking, app
actions, and durable ViewModels remain in Rust crates.

The likely product shape is:

```text
Rust core/app/net/store
Native desktop shell
Web-rendered UI
Local-only bridge
No centralized web service
```

The old web application was deleted because its architecture was wrong for the
new target, not because HTML/CSS/TypeScript are wrong as a local desktop
rendering surface.

### 2.4 The First Implementation Should Be Small

The first registry layer should cover only the primitives needed for the first
real desktop shell:

- profile
- runtime/service status
- invite exchange
- peer list
- room timeline
- message composer
- service activity
- basic commands
- first semantic tokens and behavior settings

Do not build a giant abstract design system before the app has enough surface
area to teach us.

### 2.5 Topology Should Be Progressive

Ordinary successful use should not require people to understand addresses,
peer records, forwarding roles, synchronization, or provider selection. The UI
should automate those mechanisms through the Rust-owned command path, then
surface topology, degraded states, diagnostics, and manual control when they are
needed. Convenience must not make a provider authoritative or irreplaceable.

The same progression applies to identity and invitations. Ordinary profile,
member, invite, and connection views lead with human names, goals, and outcomes.
Raw principal and device identifiers, signed invite payloads, peer records,
addresses, and manual synchronization remain reachable through explicit
advanced details for diagnosis and intervention. Those disclosures project the
same Rust-owned state and invoke the same semantic commands; they are not a
parallel authority path.

Governance follows the same rule. Role creation exposes named, bounded
permission choices; role assignment, ban, and unban controls lead with member
and role names while stable principal and role IDs remain command payloads.
The UI projects assignments and bans from the admitted governance state rather
than reconstructing authorization. Removing a ban is described precisely: it
permits a principal to use a valid invite again, but does not itself restore
membership.

The initialized default is a conversation workspace rather than a dashboard of
all registered views. Channels, the selected conversation, its composer, and
direct media remain in the workbench. Focused header surfaces expose people and
invitations, notifications, local search, and connection health. Their
underlying named views remain registered, dockable, and restorable through Edit
layout so the focused presentation does not narrow the ontology.

## 3. Primitive Categories

### 3.1 Places

A place is a stable region where views can live.

Places describe location and purpose, not implementation layout mechanics.

Initial places:

- `sidebar`
- `main`
- `inspector`
- `activity`
- `status`

Examples:

```text
sidebar      peer list, profile summary, invite exchange
main         room timeline, message composer
activity     service events, diagnostics, sync results
status       online/offline/reachability state
inspector    future selected-peer or selected-message details
```

### 3.2 Views

A view is a concrete surface that occupies a place.

Current registered views:

- `profile.summary`
- `identity.recovery`
- `runtime.status`
- `network.health`
- `product.update`
- `invite.exchange`
- `peer.list`
- `field.test`
- `channel.list`
- `member.profiles`
- `role.list`
- `message.search`
- `notification.center`
- `room.timeline`
- `message.composer`
- `call.mesh`
- `service.activity`

The default workbench keeps conversation, people, invitation, and attention
surfaces visible. Recovery, runtime, network-health, field-test, product-update,
role-management, and service-activity views start hidden but remain dockable
and are reachable through Edit layout. Product Updates is also directly
reachable from More because update discovery, verification, activation,
rollback, and release-trust review must not depend on editing the workbench.
A compact Connection surface projects the
same Rust-owned network-health rows when topology or synchronization needs
attention; it does not create a second health model in the frontend.

Transient panels and modal command surfaces preserve keyboard location: focus
moves into a newly opened surface, modal Tab navigation remains contained, and
closing returns focus to the invoking control when it still exists. Snapshot
refreshes do not make the entire application a live region; only bounded status
and alert surfaces announce changes to assistive technology.
When a successful semantic command replaces its initiating surface, the
coordinator rejects the document root as a meaningful origin and selects a
causal fallback. Fresh creation or join focuses recovery setup; successful
recovery-kit export, identity restoration, or channel creation focuses the
message composer. These are presentation destinations after Rust acceptance,
not alternate command completion state.

Installing a selected product package, activating a staged generation, rolling
back, and rotating release-signing trust all enter one modal confirmation path
whether invoked from their view or the command palette. The review names the
running/staged generation where applicable, explains whether product-generation
or future release-key authority changes, traps modal focus, and returns focus on
cancel. Missing package or trust-transition input routes palette users to the
same Product Updates surface and required field. Confirmation never substitutes
for native-kernel authentication or activation.

Portable `.voxupdate` packages and `.voxtrust` transitions lead with named file
actions. Complete JSON text remains behind explicit disclosure for text-only
handoffs. Before confirmation, bounded frontend parsing presents release,
sequence, channel, minimum-kernel, signer, and key-set-change claims as
untrusted; malformed, oversized, or unknown-format input remains explicit.
Those previews neither reject nor authorize an artifact. The original bytes
continue to the native kernel's signature, role, sequence, downgrade, format,
size, compatibility, and resulting-trust-set checks.

Command failures lead with a bounded human explanation and a concrete recovery
action. Rust-owned recovery categories travel with the same serialized command
result used by every consumer; implementation paths and error chains remain
available only under explicit technical details and never become the recovery
authority.
Ordinary correctable validation failures use `needs_input`, so invalid names,
message content, attachments, profile fields, reactions, and empty searches do
not masquerade as product defects. Authority, connectivity, home, and internal
failures retain their distinct recovery meanings.

Failed peer diagnosis and synchronization remain current-session observations
in the Rust command host rather than disappearing with an error banner. They
replace the corresponding health row with `broken`, name the affected ordinary
peer, and carry the exact stable command plus peer/device payload needed to
retry. The ordinary header counts the broken row; a successful operation against
that same peer clears it. These observations report availability only and do
not alter membership, authority, or retained facts.

Frontend-only commands still require truthful completion evidence. In
particular, `invite.copy` waits for the operating-system clipboard write before
announcing a dismissible success status. An unavailable or rejected clipboard
is a structured `needs_human` failure with a manual path through the complete
Signed invite details; absence of the browser API never counts as success.

Fresh onboarding leads with choosing a `.voxinvite` file, progressively
discloses raw JSON paste as a text-handoff fallback, and previews bounded claims
from either source:
space name and stable ID, claimed authority, expiry, and included ordinary-peer
count. It labels those values as untrusted, explains that an unbound bearer
invite may be reused until expiry or revocation, and warns about locally visible
expiry or envelope conflicts. The preview neither admits nor rejects anything;
`space.join` still sends the original bytes to Rust for signature, genesis,
governance, expiry, and bootstrap validation.

Invite creation offers bounded one-hour, one-day, seven-day, and thirty-day
expiry choices and states that an unbound bearer is not strictly single-use.
The People surface projects active invitations from Rust's admitted governance
state rather than remembering frontend actions. `space.invite.revoke` carries
the stable invite event ID through the ordinary signed-governance admission and
peer-sync path. Revocation requires an explicit alert-dialog confirmation that
names the stale-partition limitation; cancel returns focus to the originating
invite row.

An existing but unreadable local home is not presented as fresh onboarding.
Rust reports a structured `home_error`; the shell explains the damage, keeps
technical detail disclosure explicit, and requires confirmation before
`home.archiveForRecovery` moves local identity, device certificate, and SQLite
state into a private archive. The transition never deletes those files and does
not move product-update trust state. Once Rust reports a genuinely fresh home,
focus moves to `identity.recovery.restore` so the offline kit remains the one
path that preserves principal continuity.

Until a recovery-kit export succeeds, the ordinary shell shows a compact
recovery setup prompt. The durable health marker records only completion time,
never recovery bytes or their filesystem location. After export, the prompt
recedes and the Identity Recovery view remains available through Edit layout
for intentionally creating a fresh offline copy.

Views should be bound to app-layer ViewModels or commands. They should not
assemble protocol, store, sync, or network concepts directly.

### 3.3 Commands

A command is a user-invokable action with a stable ID.

Current command families:

- shell/home/runtime: `shell.refresh`, `home.init`,
  `home.archiveForRecovery`, `runtime.goOnline`,
  `runtime.goOffline`;
- admission: `space.invite.create`, `space.invite.revoke`, `space.join`,
  `invite.copy`;
- identity recovery: `identity.recovery.export`, `identity.recovery.restore`;
- channels and attention: `channel.create`, `channel.select`,
  `channel.markRead`, `channel.rotateKey`;
- messages: `message.send`, `message.edit`, `message.redact`,
  `reaction.add`, `reaction.remove`, `pin.add`, `pin.remove`,
  `attachment.add`, `message.search`, `message.open`,
  `message.composer.focus`;
- people and governance: `profile.update`, `role.create`, `role.grant`,
  `role.revoke`, `member.ban`, `member.unban`;
- calls: `call.join`, `call.signal`, `call.heartbeat`, `call.leave`;
- peers: `peer.import`, `peer.diagnose`, `peer.sync`;
- workbench/preferences: `ui.preference.set`, `ui.preferences.reset`, `workbench.layout.save`,
  `workbench.layout.reset`, `workbench.commandPalette.open`.

Commands should be reachable from more than one surface over time:

- visible buttons
- command palette
- keyboard shortcuts
- automation or scripting later

The command ID is the durable concept. The button is only one affordance.

Message reply and edit affordances remain inside the conversation surface.
Selecting Reply establishes local composer context, but the accepted action is
still `message.send` with the root event ID in its payload. Inline editing and
its Enter/save or Escape/cancel keyboard paths invoke `message.edit` only when
the person commits. Draft context is disposable presentation state; it never
becomes a parallel message or thread authority.

Mention composition leads with current display names. The composer and inline
editor insert a visible `@name`, then carry the corresponding stable peer IDs
through `message.send` or `message.edit`. A typed name resolves automatically
only when unambiguous; duplicate display names require the member picker, which
disambiguates the choice without making raw IDs the ordinary interaction.

Each projected search result and mention notification is an affordance over
`message.open`. Rust validates that the retained event belongs to an accessible
channel, marks that channel read, and returns a bounded conversation projection
anchored on the selected event—even when it predates the ordinary latest-message
window. The frontend closes the transient surface and focuses the returned
message; it does not maintain read state, reconstruct history, or infer result
validity.

Banning a member requires an explicit focused confirmation that states the
authority loss, retained-history behavior, and fresh-invite requirement before
invoking `member.ban`. Canceling or completing the action restores a stable
keyboard location in the member row. The confirmation does not predict or
project governance state; the Rust snapshot remains authoritative.

Granting or revoking a role likewise requires a focused confirmation that names
the member, role, direction of change, and human-readable permissions gained or
lost. It states that other roles remain unchanged, then invokes only
`role.grant` or `role.revoke`. Canceling or completing restores focus to the
stable role row; accepted Rust governance remains the only assignment truth.

Projected reaction and pin state determines whether the visible action invokes
the add or remove command; the frontend does not guess a toggle result. Message
deletion is a separate confirmed step that states the retained signed-tombstone
effect before invoking `message.redact`.

### 3.4 Semantic Tokens

A semantic token is a named visual meaning, not a raw color.

Initial token families:

- `app.*`
- `panel.*`
- `text.*`
- `runtime.*`
- `peer.*`
- `message.*`
- `activity.*`

Initial tokens:

- `app.background`
- `panel.background`
- `panel.border`
- `text.primary`
- `text.secondary`
- `runtime.online`
- `runtime.offline`
- `peer.reachable`
- `peer.unreachable`
- `message.own.background`
- `message.remote.background`
- `activity.info`
- `activity.error`

Each token should eventually expose:

- stable ID
- default value
- current user value
- where it is used
- whether it is user-editable
- editing surface

### 3.5 Metrics

A metric is a named size or spacing primitive.

Initial metrics:

- `sidebar.width`
- `panel.padding`
- `panel.gap`
- `message.gap`
- `message.maxWidth`
- `avatar.size`
- `activity.maxItems`

Metrics should be treated as customization primitives, not magic numbers hidden
inside components.

### 3.6 Behaviors

A behavior is a user-tunable rule.

Initial behaviors:

- `timestamps.visible`
- `timestamps.style`
- `activity.autoScroll`
- `peerList.compact`
- `sync.autoAfterImport`
- `runtime.startOnlineOnLaunch`

Behavior settings are not visual theme settings. They belong to the same
customization ontology because they shape how the app behaves for the user.

### 3.7 Renderers

A renderer is a swappable way to display a domain object.

Initial renderer concepts:

- `message.renderer`
- `peer.renderer`
- `activity.renderer`

The first implementation may have only one renderer for each concept. Naming the
renderer still matters because it creates the future extension point without
making components pretend that rendering is not a concept.

### 3.8 Editing Surfaces

An editing surface is an in-app UI that lets a user change a primitive.

Initial editing surfaces:

- command palette
- appearance/token editor
- layout/place editor
- behavior settings
- peer/display settings

The command palette and layout editor now exist. The workbench's focused
Customize surface presents everyday behavior first, with bounded choices for
values such as timestamp style. Advanced appearance and spacing remain
discoverable in the same surface with their stable semantic IDs. Token, metric,
and behavior values are persisted through the Rust preference authority, and
`ui.preferences.reset` restores the complete customization and workbench layout
defaults through that same authority. Renderer replacement remains a named
future editing surface rather than a claimed implementation.

## 4. Primitive Record Shape

Every exposed primitive should eventually answer:

- What is its stable ID?
- What kind of thing is it?
- What is the default?
- What is the current user value?
- Where is it used?
- Is it user-editable?
- What editing surface owns it?

Example:

```text
id: peer.reachable
kind: semantic_token
default: system success color
current: user preference or default
used_by: peer.list, peer.detail, diagnostic.result
editable: true
editing_surface: appearance/token editor
```

## 5. First Registry Layer

The first code implementation did not build the full UI shell first.

It added a durable ontology registry layer that feeds the Tauri-style web UI:

```text
voxelle-app
  ui ontology registry
  command registry
  default token registry
  default metric registry
  default behavior registry
  persisted user preference model
  ViewModels that reference stable primitive IDs
```

The local web UI now consumes the Rust-owned token, metric, and behavior
registries. Its Customize surface sends one typed preference command through
the local bridge; Rust validates and persists the change, and the returned
snapshot changes the rendered interface. The standalone browser fixture is
generated from the same Rust defaults. It is a read-only visual preview: it
does not simulate home, service, messaging, peer, sync, or persistence
behavior. Those commands require the real local bridge.

Places and views now name, order, and compose the visible workbench. Every view
can be dragged to any named dock, moved with its dock selector and order
buttons, hidden, restored, or reset. Rust validates and persists the complete
placement set, and the layout survives application restart and travels inside
the encrypted recovery capsule. The command palette is populated from the same
Rust-owned command records used by visible buttons and shortcuts; command scope
distinguishes shell actions from local presentation actions. Renderer selection
remains planned. Semantic tokens, metrics, behaviors, places, and views report
`editable: true` because their in-app editing paths now exist.

## 6. Framework Direction

The chosen direction is a Tauri-style desktop shell:

```text
Rust backend + web-rendered UI + local bridge
```

This is preferred over continuing to deepen the current `eframe` shell because
the customization target needs web-like styling, semantic tokens, inspectable
component boundaries, and in-app editing surfaces.

The current `voxelle-desktop` crate remains useful as a disposable operator
shell. It should not become the long-term product UI unless a later decision
explicitly reverses this direction.

## 7. Non-Goals For This Layer

This ontology does not define:

- final visual design
- final layout
- theme file format
- extension API
- marketplace or package system
- plugin security model
- complete settings UI
- every future customization primitive

Those should emerge from the registry layer and product surface as the app
becomes more concrete.

## 8. Design Rule

If a user-visible thing might reasonably be customized, first ask:

What kind of thing is this?

Only after it has a kind and a stable home should we decide how it is painted,
where it appears, or how the user edits it.
