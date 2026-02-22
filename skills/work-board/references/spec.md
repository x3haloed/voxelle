# Work Board: Contract and Workflow

## Three planes

- Evidence plane (agent-owned, append-only): `.isnad/ledger.jsonl`
- Control plane (human-owned, append-only): `.isnad/control.jsonl`
- State plane (derived, overwriteable): `.isnad/state/*`

Never edit or delete evidence/control files. Make corrections by appending records/directives that reference prior ids.

## Minimal on-disk layout

Create these paths in the target repo:

- `.isnad/ledger.jsonl`
- `.isnad/control.jsonl`
- `.isnad/state/board.json` (generated)
- `.isnad/state/board.md` (generated)
- `.isnad/state/cursors.json` (generated; stores last seen directive id(s) and last folded offsets)

## Source of truth vs derived

Recommended defaults:

- Commit and review: `.isnad/ledger.jsonl` and `.isnad/control.jsonl`
- Ignore: `.isnad/state/*` (derived, regenerate any time)

## Automation pattern (recommended)

- UIs should keep `.isnad/state/*` refreshed while running (watch/poll inputs).
- CLI fallback: run the fold in watch mode (or re-run it after changes) so the Board view stays current for wake-up hooks.

## Steering model (human)

The human steers by appending directives to the control plane:

- Set task status/priority
- Pause/resume
- Set/override current goal
- Request a snapshot/explanation
- Reject a ledger record (forces agent response + supersede flow)

The human never edits the ledger directly.

## Attention guarantee (agent receipt protocol)

On every agent turn:

1) Read control directives after `control_ack_cursor`.
2) For each new directive, append a ledger record `ack_directive` with:
   - `directive_id`
   - “what I understood”
   - “what I will do next”
3) If the directive changes priorities/goals, update the plan and cite the directive id(s).
4) If compliance is impossible or unsafe, append `cannot_comply` with reasons and a proposed alternative.
5) When done, append `complete_directive` with artifacts and verification links.

The UI should surface:

- “Pending directives” count
- “Last acknowledged directive id/time”
- “Last folded state time”

## Folding rules (derive board state)

Goal: deterministically compute the current board and task state from append-only logs.

Inputs:

- Evidence plane records (for task definitions, titles, and evidence links)
- Control plane directives (for status/priority/goal overrides and requests)

Rules:

1) A task exists if there is a `task_opened` evidence record with `task_id`.
2) Title/source-of-truth:
   - Prefer the latest evidence record that sets `task.title` (e.g., `task_opened`, `task_updated`).
3) Status:
   - Default `backlog`.
   - Apply control directives `set_status` in timestamp order; last wins.
4) Priority:
   - Default `medium`.
   - Apply `set_priority` in timestamp order; last wins.
5) Block reason:
   - Apply `block` / `pause` directives; last wins and sets status to `blocked` unless explicitly overridden later.
6) “Unread”:
   - A directive is “unread” until there is a corresponding `ack_directive` evidence record referencing its id.
7) “Done”:
   - Do not infer done from tests or commits; only `set_status=done` (human) or an explicit evidence record that marks done (agent), depending on the chosen policy.

### Ordering recommendation (do this)

For determinism and resilience to clock skew:

- Fold primarily by **append order** (file line order / read sequence), not by `ts`.
- Use `ts` for display only.

Update the rules above accordingly: “last wins” means “last by append order”.

### Task creation recommendation (straightforward fix)

Avoid “steering unknown tasks” ambiguity by supporting a control directive that creates tasks:

- `open_task` (control): creates a task proposal with `task_id` and `payload.title`.
- Agent must acknowledge `open_task` and append a corresponding evidence `task_opened` record (same `task_id`, title in `meta.title`).

Folding may display `open_task` tasks as **provisional** until evidence `task_opened` exists.

## Secrets

Do not store secrets (API keys, tokens, passwords, private keys) in `.isnad/ledger.jsonl` or `.isnad/control.jsonl`.
Store only safe references (e.g., “see 1Password item X”, “see env var name”), never the secret material.

## UI requirements (web preferred)

Minimum capabilities:

- Board columns: Backlog / Next / Doing / Blocked / Done / Rejected
- Card shows: task id, title, priority, updated time, unread directive count
- Card detail shows:
  - latest snapshot (if any)
  - pending directives + receipts
  - evidence links (files, diffs, tests)

Mutation rules:

- UI writes control directives only.
- UI regenerates derived state (or triggers regeneration).
- UI never edits evidence/control history.

Task id rules:

- UI/TUI must create new tasks only by emitting an `open_task` directive (which mints or assigns a task id).
- UI/TUI must not allow arbitrary freeform `task_id` entry for task-scoped directives; it should offer selection from existing tasks plus “Create new task”.
- If a directive somehow references an unknown `task_id`, show it as provisional and prompt to create/confirm via `open_task`.

Auto-launch:

- Prefer a single command that starts server, watches files, and opens browser tab.
- Bind to `127.0.0.1` by default.
