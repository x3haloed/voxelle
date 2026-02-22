# Wake-up hooks (context wipe bootstrap)

Goal: ensure a fresh agent session reliably “re-attaches” to the provenance system before taking meaningful actions.

## Required boot sequence (agent)

On session start (or when switching repos):

1) Discover `.isnad/`. If missing, create it (and append an `init` record).
2) Read `.isnad/state/board.md` (or generate it by folding if missing/stale).
3) Read pending directives (those without an `ack_directive` receipt).
4) Read the most recent global `snapshot` and any task snapshots referenced by “Doing/Blocked/Next”.
5) Write a new evidence record:
   - `type: snapshot` (or `resume`)
   - `claim`: what you believe the current goal + next actions are
   - `evidence`: cite board + directive ids you used
6) Only then continue with work; for any new directive, immediately write `ack_directive`.

## Recommended “attention guarantee”

Always include in your working plan:

- `last_acknowledged_directive_id`
- `pending_directives_count`

If you cannot find `.isnad/`, say so and propose scaffolding it.

## Templates

### Template: `AGENTS.md` snippet

Add to `AGENTS.md` (or equivalent agent instructions file):

```
## Work Board (required)
Before taking meaningful actions, attach to the repo’s provenance system:
1) If `.isnad/` is missing, create it and append an `init` record.
2) Regenerate `.isnad/state/board.md` from `.isnad/ledger.jsonl` + `.isnad/control.jsonl`.
3) Read pending directives in `.isnad/control.jsonl` (any without a matching `ack_directive` in the ledger).
4) Read the latest snapshots for active tasks.
5) Append a `snapshot` (or `resume`) ledger record stating your current understanding and next decision.
Then proceed; acknowledge every new directive with `ack_directive`.
```

### Template: `.github/copilot-instructions.md` snippet

If your harness supports it, add:

```
Before you start, look for `.isnad/` and attach to the Work Board system:
- Read `.isnad/state/board.md` (or regenerate it).
- Read new directives in `.isnad/control.jsonl` and acknowledge them in `.isnad/ledger.jsonl`.
- Append a `snapshot` record describing current goal and next steps with citations.
```

## UI hook (optional)

If a Layer 2 web UI exists, prefer a single command such as:

- `isnad board` (starts server, opens browser, watches files)

But the wake-up hook must not depend on the UI being available; it should fall back to folding state from JSONL.
