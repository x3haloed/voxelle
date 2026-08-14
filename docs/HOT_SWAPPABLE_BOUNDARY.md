# Hot-Swappable Product Boundary

Status: executable majority boundary implemented; packaged-native signed
activation evidence pending for this widened payload.

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

Until the packaged-native item and full preservation rerun pass, the boundary is
implemented but the final lived majority claim remains open.
