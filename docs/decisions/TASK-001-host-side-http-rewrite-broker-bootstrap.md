# Decision: host side http rewrite broker bootstrap

## Task
TASK-001

## Date
2026-03-05

## Context
Need a standalone secret broker component that can be reused across Jim-managed projects. The broker must keep credentials on host, rewrite protocol calls for sandbox workloads, and remain easy to run in local development.

## Options Considered
- Keep broker logic embedded only in `jim.ts`.
- Extract a minimal standalone CLI project with host-side grants and runtime rewrites (chosen).
- Build a daemon-first service with persistent session lifecycle and external API.

## Decision
Create `janus` as a standalone Bun TypeScript project that:
- loads grants from `.jim/secret-grants.json`,
- resolves host-side credentials from environment variables,
- auto-discovers target Git hosts from `git remote -v` (with optional allow-list override),
- starts host-local HTTP adapters that inject auth headers,
- outputs Git rewrite env and optionally executes a command with those env overrides.

## Reasoning
This keeps implementation practical for immediate dogfooding while establishing a separable broker boundary that Jim and future tools can invoke.

## Consequences
- Positive: broker behavior can evolve independently from Jim core lifecycle logic.
- Positive: host-side credential handling remains explicit and auditable.
- Positive: project has its own Jim contract context for consistent process and records.
- Negative: introduces another repository/tool surface to maintain.

## Scope
Task-specific

## Links
- Related ADR:
- Related evolution event: [20260305-104926-host-side-http-rewrite-broker-bootstrap.md](../../evolution/events/20260305-104926-host-side-http-rewrite-broker-bootstrap.md)
- Evidence (files/tests): `src/janus.ts`, `README.md`, `.jim/secret-grants.json`, `bun run src/janus.ts help`, `bun run src/janus.ts plan`
