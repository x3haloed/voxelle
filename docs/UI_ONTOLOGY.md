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
- `runtime.status`
- `network.health`
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

Views should be bound to app-layer ViewModels or commands. They should not
assemble protocol, store, sync, or network concepts directly.

### 3.3 Commands

A command is a user-invokable action with a stable ID.

Current command families:

- shell/home/runtime: `shell.refresh`, `home.init`, `runtime.goOnline`,
  `runtime.goOffline`;
- admission: `space.invite.create`, `space.join`, `invite.copy`;
- channels and attention: `channel.create`, `channel.select`,
  `channel.markRead`, `channel.rotateKey`;
- messages: `message.send`, `message.edit`, `message.redact`,
  `reaction.add`, `reaction.remove`, `pin.add`, `pin.remove`,
  `attachment.add`, `message.search`, `message.composer.focus`;
- people and governance: `profile.update`, `role.create`, `role.grant`,
  `role.revoke`, `member.ban`, `member.unban`;
- calls: `call.join`, `call.signal`, `call.heartbeat`, `call.leave`;
- peers: `peer.import`, `peer.diagnose`, `peer.sync`;
- workbench/preferences: `ui.preference.set`, `workbench.layout.save`,
  `workbench.layout.reset`, `workbench.commandPalette.open`.

Commands should be reachable from more than one surface over time:

- visible buttons
- command palette
- keyboard shortcuts
- automation or scripting later

The command ID is the durable concept. The button is only one affordance.

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

The command palette and layout editor now exist. Token, metric, and behavior
values are persisted through the Rust preference authority and are reachable
through the workbench's Customize surface. Renderer replacement remains a
named future editing surface rather than a claimed implementation.

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
