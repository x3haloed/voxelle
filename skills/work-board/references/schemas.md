# JSONL schemas (minimal)

Keep one JSON object per line.

All timestamps should be ISO-8601 UTC (e.g., `2026-02-20T20:12:45Z`).

## Evidence record (`.isnad/ledger.jsonl`)

Required fields:

- `id` (string, globally unique)
- `ts` (string, ISO-8601 UTC)
- `type` (string)

Recommended fields:

- `topic` (string; project/workstream tag)
- `task_id` (string; for task-related records)
- `parents` (string[]; ids this record follows from)
- `supersedes` (string[]; ids this record corrects)
- `claim` (string)
- `action` (string)
- `artifact` (object|string; file/diff/test output pointer)
- `evidence` (object|string; where it can be verified)
- `next_decision` (string; continue/escalate/close + rationale)
- `meta` (object; freeform)
  - Recommended: `meta.actor` (e.g., `agent`), `meta.model`, `meta.run_id`

Core `type` catalog:

- `init`
- `task_opened`
- `task_updated`
- `claim`
- `decision`
- `action`
- `test_run`
- `snapshot`
- `ack_directive`
- `cannot_comply`
- `complete_directive`
- `supersede` (or use `supersedes` on any type)

Receipt record payload conventions:

- `ack_directive`: set `meta.directive_id` and summarize understood intent in `claim`
- `cannot_comply`: set `meta.directive_id` and include constraints in `claim`
- `complete_directive`: set `meta.directive_id` and include verification in `evidence`

## Control directive (`.isnad/control.jsonl`)

Required fields:

- `id` (string, globally unique)
- `ts` (string, ISO-8601 UTC)
- `type` (string)

Recommended fields:

- `task_id` (string; optional for global directives)
- `author` (string; default `human`)
- `meta` (object; freeform)
  - Recommended: `meta.via` (e.g., `board-ui`, `tui`, `cli`), `meta.operator` (e.g., your name), `meta.host`
- `payload` (object)
- `rationale` (string)

Directive `type` catalog (suggested minimal set):

- `open_task` payload: `{ "title": "...", "status": "backlog|next|doing|blocked|done|rejected", "priority": "low|medium|high|urgent" }`
- `set_status` payload: `{ "status": "backlog|next|doing|blocked|done|rejected" }`
- `set_priority` payload: `{ "priority": "low|medium|high|urgent" }`
- `set_goal` payload: `{ "goal": "..." }`
- `pause` payload: `{ "reason": "..." }`
- `resume` payload: `{ "note": "..." }`
- `request_summary` payload: `{ "scope": "task|global", "depth": "brief|normal|deep" }`
- `reject_record` payload: `{ "record_id": "...", "reason": "..." }`
- `note` payload: `{ "text": "..." }`

## Derived board (`.isnad/state/board.json`)

Recommended fields:

- `generated_at`
- `columns`: map of column id -> list of cards
- `cards`: map of `task_id` -> card data
- `unread_directives`: map of `task_id` -> directive id list
- `last_ack_directive_ts`
- `last_ack_directive_id` (optional)
- `last_ack_control_seq` (optional; count of directives processed by receipts)

Card data (suggested):

- `task_id`, `title`, `status`, `priority`
- `updated_at`
- `updated_seq` (optional; fold-order sequence)
- `latest_snapshot_id`
- `evidence_links` (list)
