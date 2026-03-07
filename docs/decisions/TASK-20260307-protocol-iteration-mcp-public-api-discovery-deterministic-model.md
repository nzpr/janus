# Decision: mcp public api discovery deterministic model

## Task
TASK-20260307-protocol-iteration

## Date
2026-03-07

## Context
User required MCP discovery to use only public Janus API endpoints, with janusd itself explicitly deterministic/non-LLM.
Existing MCP surface exposed health/capability tools but did not provide structured protocol/resource availability plus unavailable gaps.

## Options Considered
- Keep current MCP tools only (`janus.health`, `janus.capabilities`, `janus.safety`) and let agents infer gaps.
- Allow MCP to call non-public/session endpoints for richer discovery metadata.
- Build deterministic discovery from only `GET /health` and `GET /v1/config`, and publish explicit protocol/resource availability (chosen).

## Decision
Implement a public-API-only discovery path in `janus-mcp` (Go and Rust):
- Add tool `janus.discovery`.
- Add MCP resources:
  - `janus://discovery/protocols`
  - `janus://discovery/resources`
  - `janus://discovery/summary`
- Classify available/unavailable protocol and typed resource capabilities from `/v1/config`.
- Include execution model metadata in discovery output.

Also extend janusd `/v1/config` (Go and Rust) with:
- `discovery.publicEndpoints` (`/health`, `/v1/config`)
- `executionModel` (`deterministic=true`, `llmDriven=false`, notes)

## Reasoning
- Keeps discovery auditable and constrained to public metadata endpoints.
- Gives LLM agents deterministic, explicit availability/gap data for planning without secret/session API access.
- Aligns MCP behavior with Janus safety model and external-daemon operational boundary.

## Consequences
- MCP discovery now has a stable contract for protocol/resource planning.
- `janus-mcp` remains read-only and requires external janusd lifecycle management.
- Capability catalogs in MCP must stay synchronized with daemon capability model over time.

## Scope
Task-specific

## Links
- Related ADR:
- Related evolution event: [20260307-173050-mcp-public-api-discovery-deterministic-model.md](../../evolution/events/20260307-173050-mcp-public-api-discovery-deterministic-model.md)
- Evidence (files/tests):
  - `.env.example`
  - `go/cmd/janus-mcp/main.go`
  - `go/cmd/janus-mcp/main_test.go`
  - `src/mcp_server.rs`
  - `go/cmd/janusd/main.go`
  - `src/main.rs`
  - `README.md`
  - `go/README.md`
  - `cd go && go test ./...`
  - `cargo test -q`
