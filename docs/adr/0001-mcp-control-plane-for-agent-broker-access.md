# ADR-0001: mcp control plane for agent broker access

## Status
Accepted

## Date
2026-03-05

## Context
Janus is primarily consumed by LLM agents. We need a standard and reliable way for agents to request brokered access across protocols without exposing raw credentials in prompt/tool payloads.

## Decision
Use MCP as the **control plane** for Janus, and keep protocol traffic on Janus proxy endpoints as the **data plane**.

Concretely:
- Agent calls MCP tools to request/start/stop Janus sessions.
- MCP responses return non-secret connection bundles (endpoint, env key names, file paths, TTL, lease ID).
- Actual HTTP/gRPC/SSH/database traffic goes directly to Janus adapters, not through MCP.

## Options Considered
- Direct CLI wrapping only (`janus run -- ...`).
- Custom HTTP API only for agent orchestration.
- MCP control plane + direct Janus data plane (chosen).

## Consequences
### Positive
- Aligns with agent ecosystem conventions and tool-calling UX.
- Keeps secret handling out of model-visible channels.
- Works across protocols while preserving Janus adapter model.
- Enables lease/TTL/revocation semantics at session level.

### Negative
- Requires an MCP server layer and tool contract maintenance.
- Two-plane architecture adds some operational complexity.

## References
- Related task(s): TASK-003
- Related decision notes: [TASK-002-multi-transport-secret-broker-adapters.md](../decisions/TASK-002-multi-transport-secret-broker-adapters.md), [TASK-003-janus-mcp-server-stdio-install.md](../decisions/TASK-003-janus-mcp-server-stdio-install.md)
- Related evolution events: [20260305-112326-mcp-control-plane-for-agent-broker-access.md](../../evolution/events/20260305-112326-mcp-control-plane-for-agent-broker-access.md)
- Source links: `src/janus.ts`, `src/mcp-server.ts`, `README.md`
