# Hot-Swappable Product Boundary

Status: executable maximum-admissible browser boundary implemented and verified
through signed packaged-native activation, failure recovery, rollback, and
restart on macOS; native Windows lived verification remains external.

## Claim

The maximum currently admissible Voxelle browser application behavior is owned
by the signed product generation. The installed native kernel continues
to own identity, governance, admission, durable storage, synchronization,
networking, recovery, encryption, stable semantic command/view identities, and
update integrity.

“Most” is evaluated by behavioral ownership first. Source size is a secondary
check that prevents a nominal component boundary from hiding the implementation
in its host.

## Replaceable generation ownership

The signed generation owns:

- all workbench view composition and DOM projection;
- buttons, forms, drafts, palette interaction, shortcuts, docking interaction,
  notifications, update UI, field-test UI, and error presentation;
- browser-side voice/video session coordination and rendering;
- all product CSS;
- view, command, renderer, behavior, metric, and semantic-token presentation.

The current executable component is assembled from
`web/src/product-component.js`, its four independently tested capability-helper
modules, and `web/src/styles.css`. The release generator embeds their effective
runtime bytes in the signed package. Rust verifies and selects the package, then
projects the verified component bytes and a domain-separated digest to the
stable WebView host.

## Stable host and capability adapter

The stable WebView material is intentionally bounded:

- `web/src/main.js` loads verified component bytes and serializes generation
  transitions;
- `web/src/product-component-host.mjs` compiles, mounts, disposes, restores, and
  swaps one component and stylesheet;
- `web/src/shell-client.js` exposes the single typed Rust command bridge;
- `web/index.html` supplies only the root element and failure projection.

DOM reconciliation, ontology interpretation, workbench transforms, and media
primitives are now concatenated into the verified component source rather than
injected by the stable host. Their standalone modules remain the testable source
representation, not a parallel running implementation.

## Defended native kernel

The Rust crates remain stable by design where they own security, durable truth,
or cross-client semantic compatibility. In particular, a component cannot:

- create or register a semantic shell command;
- change a command's native/frontend authority class;
- bypass typed request decoding and command serialization;
- validate or admit identity, governance, room, private-room, or media facts;
- write authoritative SQLite state directly;
- control QUIC, synchronization, encryption, recovery, or update selection
  except by requesting an existing kernel command.

This is not counted as unfinished componentization: those are the explicit
stable capabilities against which independently updatable product code runs.

## Maximum boundary fixed point

The remaining 203 browser-runtime lines cannot move into the generation without
making the generation select, authenticate, or bootstrap itself:

- the loader must exist before verified generation bytes can execute;
- the lifecycle host must retain the old component while it compiles the next
  one and must own restoration when mounting fails;
- the shell bridge must cross the OS WebView/native boundary and expose only the
  kernel's serialized command and invalidation interfaces.

Moving any of these into the payload would create a circular bootstrap or let
the replaceable code redefine its own capability boundary. The remaining Rust
application code either implements one of the defended authorities, projects
already accepted meaning without asking the frontend to reconstruct protocol
truth, or adapts those authorities to the stable command vocabulary. Under the
current truthful-system contract, this is therefore the maximum admissible
hot-swappable boundary rather than merely the first majority threshold.

## Boundary accounting

At this checkpoint the replaceable executable, helper modules, and stylesheet
contain 2,804 lines. The stable browser runtime contains 203 lines across the
loader, component lifecycle host, and native shell bridge. Excluding tests,
generated fixtures/contracts, and preview data, the signed generation therefore
owns 93.2% of browser product/runtime source by line count (2,804 of 3,007
lines).

Line count is not the primary proof. The stronger structural fact is that every
named product view renderer and all product CSS have one implementation, and
that implementation is carried by the signed generation rather than the native
installer. The host contains no parallel view renderer or recovery stylesheet.

## Required evidence

- Rust tests: signed package activation, persistence, rollback, command
  serialization, live peer service, and retained-message preservation.
- Component-host tests: executable behavior and CSS replacement, disposer
  execution, syntax failure without disruption, and remount after mount failure.
- Browser evidence: the component bytes execute and project the complete
  workbench with exactly one generation stylesheet.
- Packaged-native evidence: a signed generation with observably different
  executable behavior and CSS activates and rolls back in the installed Tauri
  app without process restart while a peer service and accepted message survive.
- Preservation suites: identity recovery/revocation, invite onboarding,
  offline-inviter forwarding, public feature, private-room recovery,
  workbench/palette, and media slices affected by the widened boundary.

## Current packaged-native evidence

Commit `3710372` produced universal macOS DMG SHA-256 `ab03b8bc`; `hdiutil`
verified its disk image, `lipo` reported `x86_64 arm64`, and strict deep code-sign
verification accepted its ad-hoc signature.

The mounted packaged app used a fresh home and completed these real UI paths:

1. initialized a principal/space, started the IPv6 peer service, and retained
   the message `survives executable component swap`;
2. installed signed sequence 4 containing malformed executable source; the
   loader left the running component intact and kernel rollback returned to the
   built-in generation while the service and message remained visible;
3. installed signed sequence 5 (package SHA-256 `b235a67c`) and changed the
   running heading to `Voxelle · Live Component 5` without process restart;
4. installed signed sequence 6 (package SHA-256 `59d2dd81`) and changed the
   running heading to `Voxelle · Live Component 6`, again preserving the online
   service and retained message;
5. explicitly rolled back to sequence 5 and observed its executable heading,
   retained message, and online service;
6. terminated and relaunched the packaged process on the same home, which
   re-verified and mounted sequence 5 and retained the message. As specified,
   the peer service restarted offline until requested online again.

Together with the workspace preservation suite and component-host failure tests,
this closes the local macOS evidence for the 93.2% browser-runtime ownership
claim. Windows native first-launch and non-loopback multi-machine evidence remain
bounded external beta gates; neither changes which code owns the component
boundary.
