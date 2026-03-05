# Evolution Event: multi transport secret broker adapters

## Timestamp
2026-03-05T11:05:58+00:00

## Trigger
Need to extend Janus beyond HTTP-only brokering to cover major transport categories used in development and production workflows, with clarified requirement that Janus operates as a standalone proxy service (one instance per user).

## Change
- Extended `src/janus.ts` with additional adapter handling in `startSecretBroker`.
- Added `grpc/grpc_header_auth` using a local HTTP/2 proxy that injects auth headers.
- Added `ssh/ssh_key_command` for host-mode SSH private key materialization and `GIT_SSH_COMMAND`.
- Added `database/postgres_pgpass` for host-mode Postgres credential brokering through `PGPASSFILE`.
- Added `filesystem/file_materialize` for host-mode secret file materialization.
- Added runtime helper utilities for target parsing, temp lifecycle, env exports, and mode parsing.
- Added persistent `serve` command and `--instance` metadata support for long-lived proxy instances.
- Changed default grant discovery path to `.janus/secret-grants.json` with legacy `.jim` fallback.
- Moved default env naming to `JANUS_*` with legacy `JIM_*` fallbacks where relevant.
- Added `.janus/secret-grants.json` default configuration.
- Updated `README.md` with standalone service usage, adapter list, grant examples, and host/container scope notes.

## Decision Link
- ADR:
- Task decision: [TASK-002-multi-transport-secret-broker-adapters.md](../../docs/decisions/TASK-002-multi-transport-secret-broker-adapters.md)

## Validation Evidence
- `bun run src/janus.ts help`
- `bun run src/janus.ts plan`
- `bun run src/janus.ts plan --workspace /path/to/project --grants /tmp/janus-multi-grants.json --client host`
- `bun run src/janus.ts plan --workspace /path/to/project --grants /tmp/janus-multi-grants.json --client container`
- `timeout 2s bun run src/janus.ts serve --instance test-user`

## Outcome
Improved

## Follow-up
- Add a transport-agnostic adapter plugin interface to reduce conditionals in `startSecretBroker`.
- Add integration tests with local protocol fixtures (HTTP/2, Postgres client command checks).
