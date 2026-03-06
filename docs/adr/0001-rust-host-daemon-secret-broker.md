# ADR-0001: rust host daemon secret broker

## Status
Accepted

## Date
2026-03-05

## Last Updated
2026-03-06

## Context
We need a model where sandboxed LLM agents can use protected external systems without ever receiving long-lived host credentials.

## Decision
Build Janus as a Rust host daemon (`janusd`) with strict capability-based APIs.

Core decisions:
1. **No MCP runtime coupling**. Janus is an external host service, not a child process next to the LLM runtime.
2. **Host-only secret custody**. Upstream credentials remain on host env/secret stores and are never returned by Janus APIs.
3. **Capability sessions**. Agents receive short-lived session tokens and scoped env wiring, not root secrets.
4. **Control plane on local Unix socket** (`/tmp/janusd-control.sock` by default, mode `0600`).
5. **Data plane**:
   - HTTP(S) proxy capability (`http_proxy`),
   - Git-over-HTTP credential injection capability (`git_http`).
6. **Typed adapter plane (no generic shell endpoint)**:
   - Postgres query adapter (`postgres_query`),
   - deployment adapters (`deploy_kubectl`, `deploy_helm`, `deploy_terraform`).
7. **No generic `/v1/exec` endpoint**. All non-proxy operations must be explicit typed adapters.
8. **Convention-over-configuration defaults**. Service starts with no args and sane defaults.
9. **Extensibility by explicit adapters**. Future protocol support is added as new typed capabilities/adapters.
10. **Optional read-only MCP companion** (`janus-mcp`) is allowed only for capability discovery and safety docs; it is not permitted to issue sessions or expose secrets/tokens/control-socket paths.

## Why This Fits Wide App Coverage
- Proxy-native traffic (HTTP, many SDK/tooling calls) is covered by capability proxy env.
- Git-over-HTTP is mediated with host-side auth injection.
- Postgres and deployment tools are covered by typed host adapters.
- New protocols can be added without exposing raw credentials to sandbox runtimes.

## Options Considered
- Keep MCP stdio orchestration model.
- Keep TypeScript runtime and extend incrementally.
- Rust host daemon with capability APIs and typed adapters (chosen).

## Consequences
### Positive
- Stronger isolation boundary: no raw secret delivery to agent-controlled code.
- Reduced attack surface by removing generic host exec from external API.
- Clear protocol growth path via explicit adapter capabilities.
- Better auditability (capability checks and adapter-level logs).

### Negative
- Typed adapters require ongoing implementation as protocol needs grow.
- Requires careful deployment so sandbox cannot access host control socket.
- Some workflows may need additional adapter ergonomics vs unrestricted shell.

## References
- Related task(s): strict host broker hardening and typed capability adapters
- Related decision notes:
- Related evolution events:
  - [20260305-145100-rust-host-daemon-reset.md](../../evolution/events/20260305-145100-rust-host-daemon-reset.md)
  - 20260306 strict capability rollout event (this task)
  - 20260306 read-only MCP companion event (this task)
- Source links: `src/main.rs`, `src/mcp_server.rs`, `README.md`, `Makefile`, `Cargo.toml`
