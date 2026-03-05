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
- Expanded `Makefile` with useful operational scripts (`install`, host shortcuts, and `check` smoke target) and retained MCP target.
- Added `mcp` script in `package.json`.

## Decision Link
- ADR: [0001-mcp-control-plane-for-agent-broker-access.md](../../docs/adr/0001-mcp-control-plane-for-agent-broker-access.md)
- Task decision: [TASK-003-janus-mcp-server-stdio-install.md](../../docs/decisions/TASK-003-janus-mcp-server-stdio-install.md)

## Validation Evidence
- `bun run src/mcp-server.ts --help`
- `bun run /tmp/janus-mcp-smoke.ts` (MCP client connect + list tools + plan + start/list/stop session)
- `make help`
- `make check`

## Outcome
Improved

## Follow-up
- Add MCP authN/authZ and per-user policy enforcement before multi-tenant deployment.
