# Evolution Event: mcp public api discovery deterministic model

## Timestamp
2026-03-07T17:30:50+00:00

## Trigger
User requested MCP discovery to rely only on Janus public APIs and required deterministic/non-LLM janusd behavior.

## Change
- Added deterministic discovery metadata to janusd `/v1/config` in Go and Rust:
  - `discovery.publicEndpoints`
  - `executionModel` with deterministic/non-LLM flags.
- Expanded Go and Rust `janus-mcp` with `janus.discovery` tool and discovery resources (`protocols`, `resources`, `summary`).
- Implemented protocol/resource availability classification and unavailable gap reporting from only `/health` and `/v1/config`.
- Added MCP tests for discovery tool/resource surface and classification function.
- Added root `.env.example` and README updates documenting MCP public-discovery contract.

## Decision Link
- ADR:
- Task decision: [TASK-20260307-protocol-iteration-mcp-public-api-discovery-deterministic-model.md](../../docs/decisions/TASK-20260307-protocol-iteration-mcp-public-api-discovery-deterministic-model.md)

## Validation Evidence
- `cd go && go test ./...`
- `cargo test -q`

## Outcome
Improved

## Follow-up
- Keep MCP capability catalogs aligned with daemon capability additions in future protocol phases.
