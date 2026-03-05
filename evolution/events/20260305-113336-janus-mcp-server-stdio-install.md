# Evolution Event: janus mcp server stdio install

## Timestamp
2026-03-05T11:33:36+00:00

## Trigger
User requested implementation so Janus can be installed directly as an MCP server for LLM agents.

## Change
- Added `src/mcp-server.ts` (stdio MCP server) using `@modelcontextprotocol/sdk`.
- Implemented MCP tools for planning and Janus serve-session lifecycle:
  - `janus_plan`
  - `janus_session_start`
  - `janus_session_list`
  - `janus_session_get`
  - `janus_session_stop`
- Added install-facing MCP config example at `mcp/janus.mcp.json`.
- Rewrote `README.md` as an operator guide covering startup, secret provisioning, safety model, MCP operation, and legal disclaimer.
- Added MIT `LICENSE` file and explicit AS-IS warranty disclaimer reference from README.
- Updated Makefile strategy to a single `start` target for host MCP startup.
- Added `mcp` script in `package.json`.
- Added rich startup CLI banners for both `janus serve` and `mcp-server` with operational status + quick-use guidance.
- Added `JANUS_NO_BANNER=1` toggle for quiet startup mode.
- Simplified `Makefile` to a single `start` target that launches host MCP server.
- Corrected startup banner title art to explicitly render `JANUS`.
- Refined MCP startup banner wording to make host-only MCP flow explicit and remove ambiguity about manual `janus serve` startup.
- Added Claude/Codex MCP config fields directly in startup banner (`mcpServers.janus`, `command`, `args`).
- Updated startup banner to render concrete copy-paste JSON using real host paths (including `src/mcp-server.ts`).
- Added explicit startup defaults block (grants paths + default env variable names).
- Reworked README to MCP-config-first usage so operator flow is unambiguous.

## Decision Link
- ADR: [0001-mcp-control-plane-for-agent-broker-access.md](../../docs/adr/0001-mcp-control-plane-for-agent-broker-access.md)
- Task decision: [TASK-003-janus-mcp-server-stdio-install.md](../../docs/decisions/TASK-003-janus-mcp-server-stdio-install.md)

## Validation Evidence
- `bun run src/mcp-server.ts --help`
- `bun run /tmp/janus-mcp-smoke.ts` (MCP client connect + list tools + plan + start/list/stop session)
- `timeout 2s bun run src/janus.ts serve --instance demo-user`
- `timeout 1s make start`
- `timeout 1s bun run src/mcp-server.ts --workspace /workspace --client host` (banner + MCP readiness output)

## Outcome
Improved

## Follow-up
- Add MCP authN/authZ and per-user policy enforcement before multi-tenant deployment.
