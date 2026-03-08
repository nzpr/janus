# Decision: wire protocol usability connect bridge

## Task
TASK-20260308-wire-protocol-usability

## Date
2026-03-08

## Context
Janus advertised multiple non-HTTP protocol capabilities (`postgres_wire`, `mysql_wire`, `redis`, etc.) but practical agent usage was incomplete.
Most CLI/database clients do not natively speak HTTP CONNECT proxy, so capability-level tunneling alone was insufficient for real workflow usability.

## Options Considered
- Keep current capability-only model and rely on external user-provided tunnel tooling.
- Reintroduce typed protocol adapters for query execution (duplicates data-plane responsibility).
- Add a generic local CONNECT bridge utility shipped with Janus for all CONNECT-capable protocols (chosen).

## Decision
Implement `janus-tunnel` binary:
- local TCP listener in jail (`--listen`),
- upstream target host/port (`--target-host`, `--target-port` or `--protocol`),
- authenticated HTTP CONNECT through Janus proxy using session token from env.

Also update Janus session env behavior:
- emit `JANUS_CONNECT_PROXY_URL` whenever session has any CONNECT-capable protocol,
- keep `HTTP_PROXY`/`HTTPS_PROXY` scoped to `http_proxy` capability only.

## Reasoning
- Preserves clean architecture: data plane remains plain protocol tunneling.
- Avoids protocol-specific duplicate adapters in control plane.
- Makes advertised protocol capabilities actually usable with common clients (`psql`, etc.) via local bridge.

## Consequences
- Wire protocol workflows now have a first-party bridging path.
- Operators must run/automate `janus-tunnel` process for clients lacking native CONNECT support.
- Documentation must clearly explain direct vs bridged protocol usage.

## Scope
Task-specific

## Links
- Related ADR:
- Related evolution event: [20260308-081903-wire-protocol-usability-connect-bridge.md](../../evolution/events/20260308-081903-wire-protocol-usability-connect-bridge.md)
- Evidence (files/tests):
  - `src/tunnel.rs`
  - `src/tunnel_main.rs`
  - `src/janusd/mod.rs`
  - `src/janusd/control.rs`
  - `src/janusd/tests.rs`
  - `README.md`
  - `Cargo.toml`
  - `Makefile`
  - `Dockerfile`
  - `cargo test -q`
