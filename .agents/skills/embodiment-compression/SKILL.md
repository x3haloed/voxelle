---
name: embodiment-compression
description: Use when software should preserve capability while occupying fewer physical machine resources, including work that may replace representations, services, runtimes, databases, operating-system facilities, native code, or hardware mappings. Establish an explicit compression-depth contract and keep embodiment fitness separate from project economics.
---

# Embodiment Compression

Engineering work seeks embodiments that preserve capability while requiring less of the machine.

The objective is not simply speed.

The objective is reducing the physical resources required to embody the same computation.

Capability remains the invariant.

Embodiment is allowed to change.

The search may descend through frameworks, runtimes, operating-system interfaces,
instruction sets, generated machine code, assembly, firmware, or specialized
hardware when the compression-depth contract permits it.

Do not assume the current stack is the natural stopping point.

## Embodiment Is Not the Project

Keep two ledgers separate.

### Embodiment ledger

Embodiment is what physically exists or occurs on machines to realize the scoped
capability:

- instructions executed
- memory occupied and moved
- persistent bytes and representations
- processes, services, runtimes, databases, and browsers required
- serialization, synchronization, copying, and network transfer
- operating-system, firmware, accelerator, and hardware mappings used
- build-time machinery only when building is part of the scoped capability

### Project ledger

Project concerns govern whether an embodiment is acceptable to produce or own,
but they are not properties of the runtime embodiment:

- engineering time
- implementation difficulty
- verification effort
- maintainability
- delivery risk and schedule
- organizational knowledge
- ecosystem familiarity
- platform-specific source or artifact duplication
- licensing or procurement constraints

A candidate may be physically superior and still be rejected by the project.
Report that as:

> Candidate A has better embodiment fitness, but violates project constraint P.

Do not report project cost as though it made the candidate consume more CPU,
memory, storage, bandwidth, latency, or energy.

Operational machinery is embodiment; operator attention is a project concern.
For example, three required database processes are embodiment, while the effort
needed to administer them is not.

## Compression Frame

Map the system in four dimensions:

- **Capability:** the externally meaningful behavior that must remain invariant.
- **Embodiment:** the structures that physically realize that capability (algorithms, layouts, representations, services, processes, hardware mappings).
- **Resource:** CPU cycles, memory, storage, bandwidth, synchronization, cache, latency, energy, hardware, or operational machinery physically consumed.
- **Fit:** how naturally the embodiment maps onto the properties of the substrate.

The goal is not "more clever."

The goal is higher capability per unit resource.

## Compression-Depth Contract

Establish the contract before running the loop. Keep its three parts explicit:

1. **Capability invariants:** behavior and tolerances that must survive.
2. **Embodiment boundary:** the deepest substrate the search or implementation
   may replace or specialize.
3. **Project constraints:** what the project is unwilling to spend, duplicate,
   depend on, operate, or give up.

Useful embodiment boundaries include:

- tune within the current implementation
- replace algorithms and representations
- collapse layers, services, processes, or data movement
- replace frameworks, managed runtimes, browsers, or databases
- replace general-purpose storage, protocol, or operating-system facilities
- use ahead-of-time native code and specialized allocators or I/O
- produce separate implementations for each OS or instruction set
- generate machine code or write assembly for known targets
- use unikernels, firmware, accelerators, FPGAs, ASICs, or bare metal

These are search depths, not a mandatory sequence. A better embodiment may skip
levels entirely.

Portability does not require one portable implementation. It may be preserved by
an artifact family, such as separate Windows x86-64, Linux x86-64, and macOS
ARM64 embodiments. Record whether multiple artifacts are allowed.

### Missing depth

For analysis or exploration, map the full plausible depth range down to native,
ISA-specific, or bare-metal embodiments. Then identify which stopping points
require which project concessions. Do not silently stop at the current framework
or at a conventional architecture.

For implementation, do not cross into a deeper embodiment class without an
explicit boundary authorizing it. If the boundary is absent and the deeper move
would materially change the runtime, persistence model, supported platforms, or
deployment shape, pause for the project to select the depth.

This prevents both failure modes:

- stopping at a familiar stack without examining deeper embodiments
- compiling everything toward bare metal when the project never authorized that depth

## Iteration Loop

### 1. Establish the compression contract

Write the capability invariants, embodiment boundary, and project constraints.

Do not merge them into one list of tradeoffs.

---

### 2. State the preserved capability

Write down exactly what must survive.

Avoid implementation language.

Examples:

> Given the same inputs, produce equivalent outputs.

> Preserve observable API behavior.

> Preserve model quality within an accepted tolerance.

Everything else becomes negotiable.

---

### 3. Measure the current embodiment

Identify where resources are consumed.

Look for:

- data movement
- translation
- serialization
- duplicated storage
- synchronization
- cache misses
- process boundaries
- allocation
- representation width
- unnecessary precision
- idle computation
- coordination

Summarize:

> Capability X currently requires embodiments A, B, and C, consuming resources R.

---

### 4. Search across the authorized depth

Rather than asking

> How do we optimize this implementation?

ask

> What other embodiment could express the same capability?

Typical moves include:

- better representations
- quantization
- canonicalization
- data layout changes
- layer collapse
- moving computation toward data
- exploiting hardware primitives
- removing translations
- composing multiple systems into one substrate
- eliminating duplicated authority
- specializing for known constraints
- replacing a general-purpose runtime with generated native code
- compiling known queries into application-specific indexes and layouts
- replacing a general-purpose database with a capability-specific durable store
- using OS-specific syscalls, hardware intrinsics, SIMD, or accelerators
- emitting distinct binaries for different OS and ISA targets
- generating machine code or using assembly for measured kernels
- moving capability into firmware, FPGA, ASIC, or bare metal

Prefer changes that remove entire categories of work instead of merely accelerating them.

Do not equate lower-level with smaller. Assembly, custom storage, or specialized
hardware are candidates whose physical resource use must still be measured.

---

### 5. Evaluate embodiment fitness

Estimate:

- capability preserved
- CPU cycles and latency
- memory and data movement
- persistent storage and representation width
- network bandwidth and serialization
- synchronization and process boundaries
- energy and hardware affinity
- required runtime and operational machinery

Rank candidates on this ledger without project concerns.

Then evaluate project admissibility separately:

- authorized depth
- engineering and verification budget
- delivery risk and schedule
- maintainability and ownership
- allowed platform or artifact duplication
- licensing, ecosystem, and operational policy

Do not allow project admissibility to rewrite the physical ranking.

Prefer embodiments that improve multiple physical resources simultaneously.

For example:

- less memory often means less bandwidth
- fewer boundaries often means lower latency
- better locality often means fewer cache misses

---

### 6. Select within the contract

Choose the physically best candidate that preserves capability and is admissible
under the explicit project constraints.

When the physically best candidate is rejected, name both outcomes:

- the physical optimum found
- the best project-admissible embodiment

Do not present a framework-level compromise as though it were the deepest
possible compression.

---

### 7. Verify capability preservation

Confirm that the preserved capability remains true.

Verify capability separately:

- correctness and equivalence
- accepted behavioral tolerances

Measure the embodiment ledger:

- performance
- memory
- storage
- bandwidth
- latency
- energy
- operational behavior

Evaluate project acceptance separately.

Do not declare success based solely on benchmarks.

Ensure the capability itself survives.

---

### 8. Repeat

Each successful embodiment changes the search space.

Lower memory enables larger batches.

Fewer boundaries expose new opportunities.

Better locality enables new algorithms.

Continue until reaching either physical termination or the explicit embodiment
boundary.

## Termination

Distinguish two kinds of stopping point.

### Physical termination

Stop the physical search when:

1. No materially smaller embodiment preserving the capability is evident.
2. Remaining resource costs arise from irreducible properties of the capability.
3. Further reduction would change the capability rather than its embodiment.

### Contractual termination

Stop the project when:

1. The next useful candidate crosses the authorized embodiment boundary, or
2. The candidate violates an explicit project constraint.

Label this as a bounded stop, not a physical optimum. State what deeper embodiment
remains plausible and what authorization or concession it would require.

## Failure Modes

| Failure | Correction |
|----------|------------|
| Speed is optimized while memory explodes | Measure multiple resource dimensions |
| Implementation is preserved instead of capability | Restate the capability in implementation-independent terms |
| Micro-optimizations dominate | Search for alternative embodiments |
| Resource reduction changes observable behavior | Tighten the preservation rule |
| A familiar framework or database is treated as the natural floor | Continue through runtime, native, ISA-specific, and bare-metal alternatives when the contract allows |
| Portability is mistaken for one shared implementation | Preserve portability with an allowed family of platform-specific artifacts |
| Engineering effort is counted as machine resource use | Move it to the project ledger and retain the physical ranking |
| A physically better candidate is rejected without explanation | Name the violated project constraint explicitly |
| Everything is driven toward machine code by default | Enforce the explicit embodiment boundary before implementation |
| Hardware-specific tuning obscures portability requirements | Record supported targets and verify every required artifact |
| Every abstraction is removed | Preserve abstractions whose resource cost is justified by flexibility, safety, or ownership |

## Guiding Question

Do not ask:

> How can this implementation run faster?

Ask:

> What embodiment would allow this capability to occupy less of the machine?

Then ask separately:

> How deep has this project authorized the embodiment to change?
