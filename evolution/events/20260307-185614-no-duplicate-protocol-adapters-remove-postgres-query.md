# Evolution Event: no duplicate protocol adapters remove postgres query

## Timestamp
2026-03-07T18:56:14+00:00

## Trigger
User requested no duplicated protocol surfaces between data plane and control plane.

## Change
- Removed Postgres typed adapter from control plane:
  - deleted `/v1/postgres/query` route
  - removed `api_postgres_query` handler
  - removed `postgres_query` capability from known capabilities and MCP resource catalog
- Kept Postgres protocol access via data-plane `postgres_wire` capability (CONNECT tunneling).
- Updated config/health metadata to reflect only deployment typed adapters.
- Removed Postgres adapter env examples and `postgresql-client` package from Docker image.
- Updated docs wording to state control plane covers only operations unavailable on data plane.

## Decision Link
- ADR:
- Task decision: [TASK-20260307-no-dup-protocols-no-duplicate-protocol-adapters-remove-postgres-query.md](../../docs/decisions/TASK-20260307-no-dup-protocols-no-duplicate-protocol-adapters-remove-postgres-query.md)

## Validation Evidence
- `cargo test -q`

## Outcome
Improved

## Follow-up
- Optionally enforce capability-policy linting that rejects future protocol duplication between proxy and typed adapters.
