# Decision: janus mcp server stdio install

## Task
TASK-003

## Date
2026-03-05

## Context
Need a directly installable MCP server process so LLM agents can orchestrate Janus without shelling into ad-hoc commands.

## Options Considered
- Keep only CLI (`janus plan/run/serve`) and require clients to wrap shell commands.
- Build a custom HTTP control API first.
- Add stdio MCP server entrypoint with Janus session tools (chosen).

## Decision
Implement `src/mcp-server.ts` using `@modelcontextprotocol/sdk` over stdio with tools:
- `janus_plan`
- `janus_session_start`
- `janus_session_list`
- `janus_session_get`
- `janus_session_stop`

Also ship install-facing docs and config examples (`README.md`, `mcp/janus.mcp.json`) plus package/make targets.

## Reasoning
LLM agent platforms already standardize on MCP tool invocation. A native MCP server is the shortest, lowest-friction path to installation and avoids brittle shell wrappers in each client.

## Consequences
- Positive: Janus can be installed as an MCP server with a single command+args config.
- Positive: Session lifecycle is explicit and broker processes are owned by the MCP server.
- Positive: Tool contract is stable for agents across IDEs and runtimes.
- Positive: Operator onboarding is clearer with startup/secrets/safety/MCP instructions in README.
- Positive: Useful Make targets reduce manual command errors during setup and operations.
- Negative: Adds an MCP SDK dependency and a persistent session state layer in-process.

## Scope
Task-specific

## Links
- Related ADR: [0001-mcp-control-plane-for-agent-broker-access.md](../adr/0001-mcp-control-plane-for-agent-broker-access.md)
- Related evolution event: [20260305-113336-janus-mcp-server-stdio-install.md](../../evolution/events/20260305-113336-janus-mcp-server-stdio-install.md)
- Evidence (files/tests): `src/mcp-server.ts`, `README.md`, `mcp/janus.mcp.json`, `package.json`, `Makefile`, `LICENSE`, `bun run src/mcp-server.ts --help`, `bun run /tmp/janus-mcp-smoke.ts`, `make help`, `make check`
