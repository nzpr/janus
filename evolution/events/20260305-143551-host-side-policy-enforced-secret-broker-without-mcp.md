# Evolution Event: host side policy enforced secret broker without mcp

## Timestamp
2026-03-05T14:35:51+00:00

## Trigger
User requested dropping MCP and redesigning Janus as an external host service that keeps secrets outside the Jim sandbox while enabling protected API access across major protocols.

## Change
- Added ADR-0002 defining server-first Janus architecture without MCP as primary integration.
- Defined control-plane/data-plane split:
  - host daemon with authenticated control API,
  - protocol adapters/proxies for HTTP/Git/Postgres/SSH/deployment tools.
- Defined security model:
  - host-only secret custody,
  - sandbox receives only non-secret capability/session handles,
  - policy allow-lists and access audit at Janus boundary.
- Defined extensibility model for future protocol support through adapter/plugin contracts.

## Decision Link
- ADR: [0002-host-side-policy-enforced-secret-broker-without-mcp.md](../../docs/adr/0002-host-side-policy-enforced-secret-broker-without-mcp.md)
- Task decision:

## Validation Evidence
- Reviewed current runtime/help/docs showing MCP stdio process model and host grant behavior:
  - `src/mcp-server.ts`
  - `src/janus.ts`
  - `README.md`
- Confirmed ADR/evolution index entries updated by automation.

## Outcome
Improved

## Follow-up
- Implement daemon control API and session capability model.
- Reposition/remove MCP runtime/docs as deprecated path.
- Add adapter contract for standardized onboarding of new protocols.
