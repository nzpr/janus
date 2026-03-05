# Jim Agent Contract

## Mission

Deliver high-velocity coding while preserving instruction-following quality and auditable decision history.

## Non-Negotiables

1. Follow repository and task instructions exactly.
2. Do research and planning before implementation on non-trivial tasks.
3. Record decisions in `docs/adr/` or `docs/decisions/`.
4. Record evolution events in `evolution/events/`.
5. Do not hide uncertainty; ask or log assumptions.

## Operating Loop

1. Read scope and constraints.
2. Research target files and write plan.
3. Implement in small slices.
4. Validate with explicit checks.
5. Write/update decision record.
6. Log evolution event with evidence.

## Talk-Only Autopilot

When the user is conversation-only:

1. Detect if a decision was made.
2. Run local scripts directly (do not ask user to run commands):
   - ADR: `bash ./scripts/auto-audit.sh adr <slug>`
   - Task decision: `bash ./scripts/auto-audit.sh task <slug> <TASK-ID>`
3. Populate created records before finishing the task.
4. Report generated file paths in the response.

## Classification Rule

- Long-term, cross-task, architectural impact -> `docs/adr/`
- Task-specific and local tradeoff -> `docs/decisions/`

## Decision Churn Control

- Default to one task decision file per `TASK-ID`.
- If the change is a deterministic correction (implementation bug, requirement clarification, no new tradeoff), update the existing task decision and linked evolution event instead of creating a new decision/event pair.
- Create a new decision note only when there is a genuinely new tradeoff, scope shift, or architecture/process direction change.

## Quality Gates

- Instruction compliance: no skipped constraints.
- Correctness: tests/checks pass or known failure documented.
- Traceability: decision + evolution event linked.
- Reversibility: changes are small and understandable.
