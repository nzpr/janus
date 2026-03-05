# Evolution Event: mcp control plane for agent broker access

## Timestamp
2026-03-05T11:23:26+00:00

## Trigger
Need a recommended integration model for LLM agents consuming Janus.

## Change
- Recorded architecture guidance to use MCP for Janus session orchestration (control plane).
- Specified that protocol traffic should flow directly to Janus proxies/endpoints (data plane), not through MCP.
- Captured rationale and tradeoffs in ADR-0001.

## Decision Link
- ADR: [0001-mcp-control-plane-for-agent-broker-access.md](../../docs/adr/0001-mcp-control-plane-for-agent-broker-access.md)
- Task decision:

## Validation Evidence
- ADR created and linked: `docs/adr/0001-mcp-control-plane-for-agent-broker-access.md`

## Outcome
Improved

## Follow-up
- Implement MCP tool contract (`janus.session.start`, `janus.session.stop`, `janus.session.inspect`).
- Add lease TTL/revocation and per-instance authorization checks.
