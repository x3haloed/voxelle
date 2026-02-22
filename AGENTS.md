## Work Board (required)
Before taking meaningful actions, attach to the repo’s provenance system:
1) If `.isnad/` is missing, create it and append an `init` record.
2) Regenerate `.isnad/state/board.md` from `.isnad/ledger.jsonl` + `.isnad/control.jsonl`.
3) Read pending directives in `.isnad/control.jsonl` (any without a matching `ack_directive` in the ledger).
4) Read the latest snapshots for active tasks.
5) Append a `snapshot` (or `resume`) ledger record stating your current understanding and next decision.
Then proceed; acknowledge every new directive with `ack_directive`.

## Local skills
This repo includes local skills under `skills/`. If a user asks to execute a skill, read its `SKILL.md` and follow its “MUST do” requirements.

