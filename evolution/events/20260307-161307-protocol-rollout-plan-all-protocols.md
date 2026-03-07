# Evolution Event: protocol rollout plan all protocols

## Timestamp
2026-03-07T16:13:07+00:00

## Trigger
User requested reverting the Postgres-only rollout note and replacing it with an all-protocol rollout plan.

## Change
- Replaced protocol planning scope from single-protocol (Postgres) to all protocol families.
- Defined rollout waves and protocol ordering.
- Defined shared acceptance gates that each protocol must satisfy before GA.
- Removed previous Postgres-only planning records from active index entries.

## Decision Link
- ADR:
- Task decision: [TASK-20260307-protocol-rollout-protocol-rollout-plan-all-protocols.md](../../docs/decisions/TASK-20260307-protocol-rollout-protocol-rollout-plan-all-protocols.md)

## Validation Evidence
- Decision record updated with rollout waves and acceptance criteria.
- Decision and event indexes updated to remove superseded Postgres-only planning record.

## Outcome
Improved

## Follow-up
- Start Wave 0 implementation task: shared non-HTTP protocol policy primitives.
- Create the first implementation task in Wave 1 (`postgres_wire`) using the shared gates.
