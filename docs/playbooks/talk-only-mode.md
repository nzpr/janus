# Talk-Only Mode Playbook

Goal: user stays in conversation; agent handles audit mechanics.

## Rules

1. Detect decisions proactively.
2. Run audit script immediately:
   - ADR: `bash ./scripts/auto-audit.sh adr <slug>`
   - Task: `bash ./scripts/auto-audit.sh task <slug> <TASK-ID>`
3. Fill created markdown records in the same task.
4. Include file links in response.

## Decision Scope Heuristic

- Use `adr` for long-lived architecture/process choices.
- Use `task` for local implementation tradeoffs.

## Correction Handling

- If you are correcting implementation to match an already-made decision (no new tradeoff), update the existing task decision/event records.
- Do not create a new task decision/event pair for deterministic rework in the same task slice.
- If the correction changes unpushed just-created commits, rewrite those commits (amend/squash) instead of adding a separate revert commit.

## Minimum Response Contract

When a decision is logged, response must include:
- decision file path
- evolution event path
- one-line summary of what was recorded
