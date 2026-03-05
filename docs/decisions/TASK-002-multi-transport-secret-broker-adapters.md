# Decision: multi transport secret broker adapters

## Task
TASK-002

## Date
2026-03-05

## Context
Janus only supported `http/git_http_auth`. We need practical brokerage coverage for major protocol classes used in delivery workflows. Scope was clarified to a standalone proxy service model (one instance per user for now), not Jim-coupled operation.

## Options Considered
- Keep Janus HTTP-only and defer all other protocols.
- Add a plugin architecture first, then implement adapters later.
- Add a direct set of high-value adapters in the existing runtime loop (chosen).

## Decision
Implement additional adapters in `src/janus.ts`:
- `grpc/grpc_header_auth` (local HTTP/2 proxy with injected authorization header),
- `ssh/ssh_key_command` (host-mode SSH key materialization + `GIT_SSH_COMMAND`),
- `database/postgres_pgpass` (host-mode Postgres `PGPASSFILE` generation),
- `filesystem/file_materialize` (host-mode secret-to-file materialization),
while retaining existing `http/git_http_auth`.

Also pivot runtime defaults to standalone service conventions:
- add persistent `serve` command for long-lived proxy instances,
- use `.janus/secret-grants.json` as default grants path (with legacy `.jim` fallback),
- use `JANUS_*` env naming in defaults/help (with legacy `JIM_*` fallback where applicable).

## Reasoning
This provides immediate multi-transport utility with concrete behavior, minimal moving parts, and explicit security boundaries. Host-only adapters avoid copying raw secrets into container environments when the target protocol cannot be safely rewritten via network proxy in this iteration. The standalone `serve` mode and `.janus` defaults align the runtime with service deployment instead of project-contract coupling.

## Consequences
- Positive: Janus now covers HTTP, gRPC, SSH workflows, PostgreSQL credentials, and file-based secret delivery.
- Positive: Existing HTTP grant behavior remains backward compatible.
- Positive: Runtime output now exposes transport-specific env keys for downstream commands.
- Positive: Janus can run as a long-lived proxy service via `serve`.
- Positive: Legacy `.jim` and `JIM_*` setups can transition incrementally.
- Negative: Some adapters are intentionally host-only (`--client host`) and are skipped for container scope.
- Negative: gRPC endpoint rewrites rely on application use of exported endpoint env variables.

## Scope
Task-specific

## Links
- Related ADR:
- Related evolution event: [20260305-110558-multi-transport-secret-broker-adapters.md](../../evolution/events/20260305-110558-multi-transport-secret-broker-adapters.md)
- Evidence (files/tests): `src/janus.ts`, `README.md`, `.janus/secret-grants.json`, `bun run src/janus.ts help`, `bun run src/janus.ts plan`, `bun run src/janus.ts plan --workspace /workspace --grants /tmp/janus-multi-grants.json --client host`, `bun run src/janus.ts plan --workspace /workspace --grants /tmp/janus-multi-grants.json --client container`, `timeout 2s bun run src/janus.ts serve --instance test-user`
