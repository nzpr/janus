# Evolution Event: mcp public api discovery deterministic model

## Timestamp
2026-03-07T17:30:50+00:00

## Trigger
User requested MCP discovery to rely only on Janus public APIs and required deterministic/non-LLM janusd behavior.

## Change
- Added deterministic discovery metadata to janusd `/v1/config`:
  - `discovery.publicEndpoints`
  - `executionModel` with deterministic/non-LLM flags.
- Expanded Rust `janus-mcp` with `janus.discovery` tool and discovery resources (`protocols`, `resources`, `summary`).
- Implemented protocol/resource availability classification and unavailable gap reporting from only `/health` and `/v1/config`.
- Added optional `JANUS_DISCOVERY_BIND` read-only HTTP listener in janusd (health/config only).
- Added optional `JANUS_PUBLIC_BASE_URL` (+ `JANUS_PUBLIC_AUTH_BEARER`) mode in `janus-mcp` so jailed MCP can use network discovery without host socket mount.
- Added tests for config env loading and MCP URL transformation helpers.
- Added root `.env.example` and README updates documenting MCP public-discovery contract.

## Decision Link
- ADR:
- Task decision: [TASK-20260307-protocol-iteration-mcp-public-api-discovery-deterministic-model.md](../../docs/decisions/TASK-20260307-protocol-iteration-mcp-public-api-discovery-deterministic-model.md)

## Validation Evidence
- `cargo test -q`

## Outcome
Improved

## Follow-up
- Keep MCP capability catalogs aligned with daemon capability additions in future protocol phases.
