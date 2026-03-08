# Decision: postgres sidecar credential injector

## Task
TASK-20260308-postgres-sidecar

## Date
2026-03-08

## Context
User requires jailed LLM operation with no secrets in the LLM process while still using developer tooling such as `psql`.
Current Janus capability model provides policy + transport tunneling, but PostgreSQL client auth still required credential material at client side.

## Options Considered
- Keep generic tunnel only and require secret in jailed client environment.
- Add typed SQL execution adapter in control plane (duplicates protocol path, reduces tool compatibility).
- Add protocol-aware Postgres auth sidecar that injects upstream credentials while preserving native `psql` workflow (chosen).

## Decision
Implement `janus-pg-sidecar` binary:
- local Postgres-compatible endpoint for jailed client (`psql` connects to localhost),
- upstream connection via Janus authenticated CONNECT tunnel,
- sidecar performs upstream Postgres authentication using configured secret (`JANUS_PG_PASSWORD` / `--upstream-password`),
- supports PostgreSQL auth methods: cleartext password, md5, SCRAM-SHA-256.

## Reasoning
- Meets hard requirement: no DB secrets in LLM process.
- Preserves developer workflow with existing client tools (`psql`) instead of custom adapter APIs.
- Keeps Janus architecture coherent: control/policy in Janus, protocol auth injection in explicit trusted sidecar component.

## Consequences
- Postgres becomes end-to-end usable for jailed agents without exposing DB password to LLM process.
- Sidecar process is now part of trusted runtime and must be deployed/operated explicitly.
- Other wire protocols may need equivalent credential sidecars depending on their auth model.

## Scope
Task-specific

## Links
- Related ADR:
- Related evolution event: [20260308-085133-postgres-sidecar-credential-injector.md](../../evolution/events/20260308-085133-postgres-sidecar-credential-injector.md)
- Evidence (files/tests):
  - `src/pg_sidecar.rs`
  - `src/pg_sidecar_main.rs`
  - `src/tunnel.rs`
  - `src/janusd/mod.rs`
  - `Cargo.toml`
  - `Makefile`
  - `Dockerfile`
  - `README.md`
  - `.env.example`
  - `cargo test -q`
