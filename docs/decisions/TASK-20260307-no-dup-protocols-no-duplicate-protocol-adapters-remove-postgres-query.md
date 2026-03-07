# Decision: no duplicate protocol adapters remove postgres query

## Task
TASK-20260307-no-dup-protocols

## Date
2026-03-07

## Context
User required no protocol duplication: plain protocol tunneling should be on data plane, and control plane should only expose operations not available on data plane.
`postgres_query` duplicated `postgres_wire` capability coverage.

## Options Considered
- Keep both `postgres_wire` and `postgres_query`.
- Keep `postgres_query` and drop `postgres_wire`.
- Keep `postgres_wire` for Postgres access and remove `postgres_query` typed adapter (chosen).

## Decision
Remove duplicated Postgres control-plane adapter path:
- delete `/v1/postgres/query` route and handler,
- remove `postgres_query` capability from known/typed capabilities,
- remove Postgres adapter discovery metadata from MCP,
- remove Postgres adapter environment defaults and container dependency (`postgresql-client`),
- keep `postgres_wire` in data plane for Postgres protocol access.

## Reasoning
- Enforces a single source of protocol access for Postgres (data-plane tunnel).
- Reduces policy ambiguity and duplicate maintenance surface.
- Keeps control-plane adapters focused on non-data-plane operations (deployment tooling).

## Consequences
- Postgres access now requires `postgres_wire` capability via proxy CONNECT path.
- Clients depending on `/v1/postgres/query` must migrate.
- Docker image is leaner without `psql` package.

## Scope
Task-specific

## Links
- Related ADR:
- Related evolution event: [20260307-185614-no-duplicate-protocol-adapters-remove-postgres-query.md](../../evolution/events/20260307-185614-no-duplicate-protocol-adapters-remove-postgres-query.md)
- Evidence (files/tests):
  - `src/janusd/mod.rs`
  - `src/janusd/control.rs`
  - `src/janusd/adapters.rs`
  - `src/janusd/tests.rs`
  - `src/mcp/discovery.rs`
  - `src/mcp/mod.rs`
  - `README.md`
  - `.env.example`
  - `.env.docker.example`
  - `Dockerfile`
  - `cargo test -q`
