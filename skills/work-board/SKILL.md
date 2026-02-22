---
name: work-board
description: Maintain an append-only work log (isnad-style provenance ledger) with a human control inbox (directives + receipts) and a derived Work Board UI (web or TUI) for steering, auditing, and prioritization without editing evidence.
---

# Work Board

## Goal

Use an isnad-style provenance ledger for agent work while giving a human a fast, interactive way to steer (priority/status/goal changes) without editing the ledger or reading raw chains.

Implement this as three planes:

- **Evidence plane (agent-owned, append-only):** ledger records of claims/actions/decisions/tests with artifacts and verification links.
- **Control plane (human-owned, append-only):** directives the agent must read and acknowledge (priority changes, pause/resume, requests, rejections).
- **State plane (derived, overwriteable):** summaries and a kanban-like board view generated from evidence + control.

We call the derived Layer 2 UI the “Board”, but it is strictly derived; it never edits the ledger.

## MUST do (otherwise this skill fails)

To make `$work-board` actually work end-to-end, you MUST do all three:

1) **Implement the Work Board system in the repo:** create `.isnad/` and start writing the evidence/control logs (see **Quick Start**).
2) **Install a wake-up hook:** modify `AGENTS.md` (ok to ask permission) or your harness equivalent so future sessions always re-attach to `.isnad/` (see **Wake-up hook** and `references/bootstrap.md`).
3) **Offer (and preferably build) a Layer 2 UI:** run or implement the web/TUI Board so humans can steer without reading raw logs (see **Build Layer 2**; for Rust, use `cargo run -p voxelle-board -- serve`).

If you skip any of these, you will create workflow gaps (no persistence across wipes, no human steering surface, or no provenance), and the system becomes unreliable/useless.

## Invariants

- Never edit or delete evidence-plane records; correct by appending a `supersede` record referencing prior record ids.
- Never treat control-plane directives as evidence; treat them as intent/commands.
- Make the agent’s attention auditable: every directive must get a receipt record in the ledger.
- Make human steering “low-bandwidth but high-leverage”: a few directive types should cover most steering.

## Quick Start (recommended workflow)

1) Create a `.isnad/` workspace in the target repo (ledger/control/state).
2) Append evidence records for every meaningful action/decision.
3) Read and acknowledge directives on every turn.
4) Regenerate the board view after any directive or task update.
5) Offer a Layer 2 UI:
   - Prefer a local web app that auto-opens a browser tab.
   - Provide a TUI fallback with the same directive-writing semantics.

Use bundled scripts as a reference implementation, or reimplement in any language.

## Minimal adoption mode (early repo)

If the repo is early and you want low ceremony:

1) Scaffold `.isnad/`.
2) Create 3–7 initial tasks via `open_task` (control).
3) Fold state and open the Board view.
4) Stop. Do not over-log. Only start full receipts/snapshots when real work begins.

Use this to bootstrap coordination without generating noise.

## Standard operation mode (later repo)

When the system is in active use:

- Write receipts: acknowledge every new directive with `ack_directive` evidence records.
- Write snapshots: when asked “what’s going on?” (or at natural checkpoints), append `snapshot` evidence records per task (or global).
- Keep the Board derived and disposable; keep ledger/control as the source of truth.

## Wake-up hook (for context wipes)

Install a “wake-up hook” so a fresh agent session reliably:

- discovers `.isnad/`
- reads pending directives
- reads the latest derived board/snapshots
- writes a `snapshot` or `resume` evidence record before acting

Preferred locations (pick what your harness supports):

- `AGENTS.md`
- `.github/copilot-instructions.md`
- Repo-specific agent instructions file

Use the templates in `references/bootstrap.md`.

## Build Layer 2 (web preferred, TUI supported)

### Define the contract first

Before writing UI, implement:

- **File layout:** `.isnad/ledger.jsonl`, `.isnad/control.jsonl`, `.isnad/state/*`
- **Schemas:** keep records/directives stable and versioned.
- **Fold algorithm:** deterministically derive board columns and per-task “current state”.
- **Receipt protocol:** guarantee the agent reads control and writes receipts in evidence.

Load details from `references/spec.md` and `references/schemas.md`.

### Implement the web UI (preferred)

Implement a small local app that:

- Watches `.isnad/ledger.jsonl` and `.isnad/control.jsonl`
- Shows a board view (Backlog / Next / Doing / Blocked / Done / Rejected)
- Writes directives when the human moves a card or changes priority
- Shows “unacknowledged directives” and the last acknowledgement cursor
- Never edits `.isnad/ledger.jsonl`

If possible, auto-open the browser on launch and bind only to `127.0.0.1`.

Automation expectation:

- When the web UI is open, keep derived state refreshed (watch files or poll) so humans never need to manually run the fold step.

### Implement the TUI (fallback)

Implement a TUI that:

- Shows the same columns and card details
- Uses keybindings to emit directives (set status/priority, pause/resume, request summary)
- Displays “last ack cursor” and “pending directives” prominently

### Make steering reliable

On every agent turn, do this (and cite directive ids in the plan changes):

1) Read new directives since the last `control_ack_cursor`.
2) Append `ack_directive` records in the ledger for each directive id.
3) Either comply (and append evidence records for actions/tests), or append `cannot_comply`.
4) When satisfied, append `complete_directive` with artifacts/tests.

## Use in practice

- Use the board for steering.
- Use snapshots for comprehension: when asked “what’s going on?”, append a `snapshot` record for the task or global scope.
- When a human disputes reality, do not edit the ledger; append `reject_record` (control) and then `supersede` (evidence) with a clear chain of citations.

## Bundled resources

- `references/spec.md`: the contract, folding rules, and UI requirements.
- `references/schemas.md`: JSONL schemas and directive/record type catalog.
- `references/bootstrap.md`: wake-up hook templates for agent harnesses.
- Rust reference implementation (preferred when avoiding Python):
  - `cargo run -p voxelle-board -- init` (scaffold `.isnad/`)
  - `cargo run -p voxelle-board -- fold` (regenerate `.isnad/state/board.*`)
  - `cargo run -p voxelle-board -- serve` (local web UI; writes control only)
  - `cargo run -p voxelle-board -- append-directive` (CLI append control directive)
  - `cargo run -p voxelle-board -- append-ledger` (CLI append evidence record)
  - `cargo run -p voxelle-board -- ack-directives` (append `ack_directive` receipts)
