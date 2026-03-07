# Decision: mcp public api discovery deterministic model

## Task
TASK-20260307-protocol-iteration

## Date
2026-03-07

## Context
User required MCP discovery to use only public Janus API endpoints, with janusd itself explicitly deterministic/non-LLM.
Existing MCP surface exposed health/capability tools but did not provide structured protocol/resource availability plus unavailable gaps.
Later clarification required sandboxed MCP operation without mounting host control socket into jail.

## Options Considered
- Keep current MCP tools only (`janus.health`, `janus.capabilities`, `janus.safety`) and let agents infer gaps.
- Allow MCP to call non-public/session endpoints for richer discovery metadata.
- Build deterministic discovery from only `GET /health` and `GET /v1/config`, and publish explicit protocol/resource availability (chosen).

## Decision
Implement a public-API-only discovery path in Rust `janus-mcp`:
- Add tool `janus.discovery`.
- Add MCP resources:
  - `janus://discovery/protocols`
  - `janus://discovery/resources`
  - `janus://discovery/summary`
- Classify available/unavailable protocol and typed resource capabilities from `/v1/config`.
- Include execution model metadata in discovery output.
- Add optional transport mode for jailed MCP:
  - `JANUS_PUBLIC_BASE_URL` (+ optional `JANUS_PUBLIC_AUTH_BEARER`) for HTTP discovery.
  - keep unix-socket mode as default when public URL is unset.

Also extend Rust janusd `/v1/config` with:
- `discovery.publicEndpoints` (`/health`, `/v1/config`)
- `executionModel` (`deterministic=true`, `llmDriven=false`, notes)
- and add optional read-only discovery listener:
  - `JANUS_DISCOVERY_BIND` serves only `GET /health`, `GET /v1/health`, `GET /v1/config`.

## Reasoning
- Keeps discovery auditable and constrained to public metadata endpoints.
- Gives LLM agents deterministic, explicit availability/gap data for planning without secret/session API access.
- Aligns MCP behavior with Janus safety model and external-daemon operational boundary.

## Consequences
- MCP discovery now has a stable contract for protocol/resource planning.
- `janus-mcp` remains read-only and requires external janusd lifecycle management.
- Sandboxed MCP can use network discovery without host control-socket mount.
- Capability catalogs in MCP must stay synchronized with daemon capability model over time.

## Scope
Task-specific

## Links
- Related ADR:
- Related evolution event: [20260307-173050-mcp-public-api-discovery-deterministic-model.md](../../evolution/events/20260307-173050-mcp-public-api-discovery-deterministic-model.md)
- Evidence (files/tests):
  - `.env.example`
  - `.env.docker.example`
  - `src/mcp/mod.rs`
  - `src/janusd/control.rs`
  - `src/janusd/mod.rs`
  - `src/janusd/tests.rs`
  - `README.md`
  - `cargo test -q`
