# Evolution Event: postgres sidecar credential injector

## Timestamp
2026-03-08T08:51:33+00:00

## Trigger
User required jailed LLM workflow where secrets are prohibited in LLM process, specifically for PostgreSQL developer tooling (`psql`).

## Change
- Added new `janus-pg-sidecar` binary.
- Implemented PostgreSQL startup handling for local client endpoint.
- Implemented upstream Postgres auth injection in sidecar for:
  - cleartext password auth,
  - md5 password auth,
  - SCRAM-SHA-256 auth.
- Connected sidecar upstream traffic through Janus authenticated CONNECT proxy path.
- Added unit tests for startup parsing, proxy URL parsing, and md5 password generation.
- Added build/deploy integration (`Cargo.toml`, `Makefile`, `Dockerfile`).
- Updated docs (`README.md`, `.env.example`) with zero-secret Postgres sidecar usage pattern.

## Decision Link
- ADR:
- Task decision: [TASK-20260308-postgres-sidecar-credential-injector.md](../../docs/decisions/TASK-20260308-postgres-sidecar-credential-injector.md)

## Validation Evidence
- `cargo test -q`

## Outcome
Improved

## Follow-up
- Add equivalent credential sidecars for other auth-heavy protocols where zero-secret jailed clients are required.
