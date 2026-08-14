# Live Product Upgrade Contract

Status: accepted implementation contract for the beta path.

## Central truth

The installed native kernel verifies a signed, content-addressed product
generation before it can become active. A generation is first parsed and
validated without authority, then one kernel-owned active-generation pointer
is changed atomically; failure preserves or restores the previous generation
without interrupting identity, retained facts, peer service, or the ordinary
semantic command authority.

GitHub Releases is the initial distribution location. It is not an update
authority: packages and release manifests are authenticated by an Ed25519
release root embedded in the kernel, may be downloaded from mirrors or ordinary
peers, and are rejected independently of their transport source.

## Defended preservation envelope

This change preserves the following statements from
`TRUTHFUL_SYSTEM_CONTRACT.md`:

- **Central Truth:** durable principal identity, independently validated facts,
  ordinary-peer retention and replication, and operation without a privileged
  Voxelle service remain unchanged.
- **Trust And Authority:** no release host or update provider decides identity,
  governance, event validity, storage, routing, encryption, or update
  integrity. Signed artifacts remain mirrorable.
- **Topology Preservation:** protocol acceptance, SQLite retention, sync, QUIC,
  the application command host, and stable semantic command/view IDs remain
  Rust-owned authorities until a later complete verified collapse replaces a
  named authority.
- **UI ontology and authority boundary:** buttons, shortcuts, palette entries,
  and automation continue to use the same semantic command IDs; a generation
  may change product presentation but cannot invent an executable shell command
  or change a command's authority class.
- **Install and integrity story:** native macOS and Windows artifacts remain
  installable without paid vendor-signing accounts, while package authenticity
  is verified independently of GitHub transport and adjacent checksums.

Representative preservation evidence after every widened generation boundary
must include the affected identity/recovery, invite onboarding, offline-inviter
forwarding, public feature, private-room/recovery, workbench/palette, and media
slices named by `AGENTS.md`.

## Accepted revisions

- The current contract's checksum-only release workflow is revised. SHA-256
  remains corruption and content-address evidence, but an Ed25519 signature
  rooted in the installed kernel becomes the update authenticity authority.
- GitHub Actions release automation is retired for the beta path. Releases are
  built, signed, verified, and published with explicit local commands. This is
  a project-operation decision, not a protocol or product authority.
- Product generations may be activated without restarting the native process.
  The initial replaceable generation is the Rust-owned UI/product ontology.
  Later slices may widen this boundary only after recording and verifying the
  newly preserved authority and resource handoff.

## Explicit exclusions

- No arbitrary native Rust dynamic-library unloading or stable Rust ABI is
  claimed.
- A disposer does not reverse already emitted network facts, accepted SQLite
  transactions, external files, media disclosure, or other effects outside the
  kernel-owned rollback boundary.
- The beta does not silently install updates. Acquisition may be automatic, but
  verification, staged details, activation status, failure, and rollback remain
  observable and user-controllable.
- Release signing does not remove Gatekeeper or SmartScreen warnings and does
  not claim Apple or Microsoft publisher identity.
- A local same-user compromise that can replace the running kernel or read the
  release signing secret remains outside the updater's protection.

## Current topology

Before this change:

- **Authorities:** Rust event admission, SQLite, sync, QUIC, command host, and
  compiled UI ontology; GitHub plus an adjacent checksum is the practical
  artifact selection path.
- **Representations:** compiled Rust ontology, generated TypeScript shell
  contract, generated JavaScript ontology fixture, native installers, and
  `SHA256SUMS.txt`.
- **Boundaries:** native Rust process, OS WebView, SQLite, optional headless
  process, and release host.
- **Coordination edges:** Rust snapshot to WebView rendering; package scripts to
  release artifacts; user checksum comparison to installation.

The first compression removes the compiled ontology as the only product
generation representation and removes GitHub/checksum adjacency as update
authenticity. The surviving authorities are the kernel verifier, the stable
command/view identity inventory, and one atomically selected signed generation.

## Generation transaction

The kernel owns this sequence:

1. fetch only the fixed GitHub `releases/latest/download/VOXELLE-RELEASE.json`
   transport location with bounded redirects, response time, and bytes;
2. parse and authenticate that manifest against the installed release roots;
3. select exactly one signed `product-update` artifact for target `any` and
   fetch its signed filename from the same fixed release location;
4. require the downloaded byte length and SHA-256 to equal the authenticated
   manifest before parsing the update package;
5. bound package bytes and parse the versioned envelope;
6. verify the package signer, domain-separated canonical signature, and exact
   release ID, channel, and sequence agreement with the manifest;
7. reject incompatible kernel versions and non-monotonic release sequences;
8. stage the exact verified bytes by content hash and persist a staged pointer;
9. parse and semantically validate the generation without activating it;
10. require a separate explicit activation command;
11. atomically replace the active pointer while retaining the previous pointer;
12. publish one snapshot invalidation after activation;
13. permit explicit rollback to the still-verified previous package;
14. on restart, re-verify active and staged packages before use and fall back to the
    previous verified generation or built-in recovery generation if necessary.

Only the kernel writes active/previous pointers. Generation payloads never
select themselves. The package store retains only the generations referenced by
active, previous, and staged pointers; unreferenced content-addressed packages
are removed. Discovery, download, staging, activation, failure, discard, and
rollback are distinct observable states.

## Release-root transition

Release authority changes use a separate `voxelle-release-trust-transition/v1`
document, never a product-generation field. Each transition:

- has the next exact monotonic trust sequence;
- is signed under a domain separate from packages and release manifests by a
  key trusted before the transition;
- may add bounded public Ed25519 roots and retire bounded existing roots;
- cannot add and remove the same ID, replay a sequence, remove an unknown key,
  or leave the kernel with no release root;
- is retained as exact signed bytes in an append-only transition chain and
  replayed from embedded roots on restart.

The embedded set separates ordinary **release** keys from an offline
**recovery** key. Release keys may sign manifests, generations, and trust
transitions. The recovery key may sign trust transitions only. Live transitions
may add or retire release keys but cannot add or retire embedded recovery keys;
changing that recovery root requires a newly installed native kernel. Thus loss
of local transition state falls back to a capability that can recover release
authority without also becoming ordinary package-signing authority.

GitHub may carry `.voxtrust` bytes but cannot authorize or order them. Applying
a transition is an explicit native command. The transition log is local update
authority state, distinct from product generations and user data.

## Initial generation boundary

Version 1 carries the complete UI ontology presentation for existing stable
places, views, commands, semantic tokens, metrics, behaviors, and renderers.
Activation validation requires:

- every kernel-known place, view, and command ID appears exactly once;
- no unknown shell command is introduced;
- each command retains its kernel-owned shell/frontend authority class;
- every view resolves to a known place;
- user-persisted layout and appearance preferences are re-applied by stable ID;
- bounded strings, collection sizes, numeric metrics, and reference IDs;
- the generated snapshot remains serializable through the existing shell
  contract.

This generation changes presentation and discoverability, not protocol
admission or command execution.

## Embodiment-depth contract

### Capability invariants

The application remains a one-process native Rust product with one OS WebView
and SQLite; network service, identity, retained state, current calls, and
in-flight command serialization survive a product-generation activation.

### Authorized boundary

This path may replace representations, libraries, frontend organization, and
in-process generation scheduling. It may add a small Rust verification and
generation manager. It may not add a mandatory daemon, hosted authority,
bundled browser, custom database, native plugin ABI, kernel component, or
specialized hardware without a later accepted revision.

### Project constraints

- Rust remains the kernel/runtime and the CLI stays independently testable.
- macOS Apple Silicon, macOS Intel, and Windows artifacts remain supported.
- No paid vendor-signing account or formal CI/CD is required.
- GitHub Releases is the initial publication surface; `gh`-driven publication
  is manual and packages remain independently verifiable after mirroring.
- The signed manifest carries the `beta` channel. A beta GitHub Release must
  nevertheless be published as the repository's latest ordinary release, not
  with GitHub's **pre-release** flag, because GitHub excludes pre-releases from
  the fixed `/releases/latest/download/...` transport path used by installed
  kernels.
- The private release signing key is never committed, printed, logged, included
  in an artifact, or made available to product runtime code.

## Evidence horizon and beta gate

Semantic evidence requires a real signed package to alter the native snapshot's
product generation while stable command execution continues through the same
Rust host. Operational evidence requires invalid signature, wrong signer,
tamper, rollback, downgrade, incompatible kernel, interrupted pointer write,
restart, corrupt active package, and concurrent command/activation probes.
Lived evidence requires the packaged native app to show available, staged,
active, failed, and rolled-back states and to preserve real workbench input and
peer operation across activation.

Beta additionally requires manually produced macOS and Windows installers, a
signed release manifest, local verification from a clean download directory,
publication to a GitHub Release, readback of the published assets, and the
existing field-test evidence. Platform claims remain bounded where the actual
platform artifact has not been executed.

## Current release evidence

The current macOS preview slice was published on 2026-08-14 as
[`v0.1.0-beta.2`](https://github.com/x3haloed/voxelle/releases/tag/v0.1.0-beta.2),
an ordinary latest release whose signed manifest declares the `beta` channel.
The tag resolves to commit `5b4e38c90464f6612a302ca3dca08d755da8689d`.

- GitHub readback through `/releases/latest/download` returned the 772-byte
  signed manifest, 15,134,373-byte universal DMG, and 32,708-byte product
  generation. Clean-directory verification authenticated the manifest and
  both listed artifacts.
- The read-back DMG passed `hdiutil verify`; its executable is a universal
  `x86_64 arm64` Mach-O, and `codesign --verify --deep --strict` accepted its
  ad-hoc hardened-runtime signature.
- The packaged native app discovered sequence 2 from the public fixed path,
  staged it without activation, activated it in-process as a signed source,
  then rolled back to the retained signed sequence 1. This proves the
  multi-generation upgrade and rollback chain rather than only the built-in
  recovery fallback.
- The native semantic-command test creates A, joins B, imports B's ordinary
  endpoint record, signs a C invite containing A and B as bounded bootstrap
  hints, takes A offline, and joins C through B with retained history visible.

This is a bounded macOS preview claim, not the complete cross-platform beta
gate. A Windows installer built and exercised on Windows, the existing
multi-machine field test, and protected offline relocation of both signing
secrets remain required before this contract may claim full beta readiness.
