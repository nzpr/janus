# ADR-0001: rust host daemon secret broker

## Status
Accepted

## Date
2026-03-05

## Context
We need a security model where Jim/LLM runs sandboxed without direct access to production credentials, while still being able to call protected APIs and services across development/production protocols.

## Decision
Build Janus as a **Rust host daemon** (`janusd`) and drop MCP from runtime architecture.

Core decisions in this ADR:
1. **No MCP control path** for Janus runtime. Janus is an external host service, not a child process of the LLM client.
2. **Host-only secret custody**. Credentials remain in host env/secret stores and are never returned to sandbox clients.
3. **Capability sessions**. Sandboxed clients receive short-lived session env values (proxy capabilities), not root secrets.
4. **Control plane via Unix socket API** by default (`/tmp/janusd-control.sock`) with restrictive file permissions.
5. **Data plane**:
   - first-class proxy for HTTP(S) and Git-over-HTTP,
   - host-exec adapter path for tooling/protocols that are not yet transparently proxyable (for example `psql`, `kubectl`, `terraform`, `ssh`).
6. **Convention-over-configuration defaults**. Service starts with no args and sensible defaults.
7. **Extensibility by adapters**. Additional protocols are added as explicit adapters without changing the secret-isolation model.

## Why This Fits Wide App Coverage
This model is suitable for a broad range of apps because:
- many tools already respect proxy env conventions,
- Git-over-HTTP can be mediated via rewrite/proxy policy,
- non-proxy-friendly tools can be covered through controlled host-exec adapters,
- future protocols can be added without exposing credentials to sandbox runtimes.

## Options Considered
- Keep MCP stdio orchestration model.
- Keep TypeScript runtime and extend incrementally.
- Move to Rust host daemon with externalized control/data planes (chosen).

## Consequences
### Positive
- Stronger secret isolation boundary for sandboxed agents.
- Single host service can govern multiple protocol families.
- Better operational hardening surface (resource limits, auditing, service management).
- Clear migration path for future protocol adapters.

### Negative
- Requires migration from previous TS/MCP tooling/docs.
- Some protocols still need dedicated adapters for fully transparent proxy semantics.
- Host-exec adapters add policy complexity and require strict allowlists.

## References
- Related task(s): reset to rust-first architecture
- Related decision notes:
- Related evolution events: [20260305-145100-rust-host-daemon-reset.md](../../evolution/events/20260305-145100-rust-host-daemon-reset.md)
- Source links: `src/main.rs`, `Cargo.toml`, `README.md`, `Makefile`
