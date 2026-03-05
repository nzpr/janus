# ADR-0002: host side policy enforced secret broker without mcp

## Status
Accepted

## Date
2026-03-05

## Context
Current MCP integration is stdio-based and typically runs as a child process of the LLM client runtime. This conflicts with the required security model:
- secrets must remain on host and never be colocated with sandboxed Jim runtime,
- Janus must run independently as host infrastructure,
- protected access should work across major protocols (HTTP/Git/Postgres/deployment tooling) and remain extensible.

## Decision
Adopt a **server-first Janus architecture** and remove MCP as the primary integration path.

Janus will run as a long-lived host daemon and provide:
- a host-only control API (Unix socket by default, optional mTLS TCP) for session/capability issuance,
- protocol adapters as data-plane proxies/wrappers (HTTP/Git/Postgres/SSH/deployment tools),
- static host policy configuration with least-privilege grants and allow-lists,
- short-lived capability tokens and fully audited access events.

Sandboxed Jim receives only non-secret runtime handles (proxy endpoints, ephemeral capability token references, rewritten env targets), never root credentials.

## Options Considered
- Keep MCP stdio control plane and continue client-launched process model.
- Build Janus as standalone host daemon with native control API and protocol adapters (chosen).
- Expose secrets directly in sandbox env with tool-level restrictions.

## Consequences
### Positive
- Aligns with strict secret isolation: credentials remain host-resident.
- Makes Janus deployment independent from agent runtime/process model.
- Supports protocol growth through adapter/plugin boundary without reworking agent integration.
- Centralizes policy enforcement, audit logging, and rotation at host service boundary.

### Negative
- Requires designing and maintaining a dedicated control API/authz model.
- Some tools/protocols need wrapper/proxy integration work (for example Postgres and deployment CLIs).
- Migration cost from current MCP tool flow to daemon/session workflow.

## References
- Related task(s): TASK-002, TASK-003
- Related decision notes: [TASK-002-multi-transport-secret-broker-adapters.md](../decisions/TASK-002-multi-transport-secret-broker-adapters.md), [TASK-003-janus-mcp-server-stdio-install.md](../decisions/TASK-003-janus-mcp-server-stdio-install.md)
- Related evolution events: [20260305-143551-host-side-policy-enforced-secret-broker-without-mcp.md](../../evolution/events/20260305-143551-host-side-policy-enforced-secret-broker-without-mcp.md)
- Source links: `src/janus.ts`, `src/mcp-server.ts`, `README.md`
