# Decision: rust modularization split monolith files

## Task
TASK-20260307-modularization

## Date
2026-03-07

## Context
User requested modular code instead of very large single-file binaries, then explicitly asked for each protocol to live in its own file.
Rust binaries had monolithic sources:
- `src/main.rs` (~1.8k lines)
- `src/mcp_server.rs` (~680 lines)

## Options Considered
- Keep monolithic files and only add comments/sections.
- Split only binary entrypoints but keep monolithic module files.
- Split by concerns into module directories while preserving behavior and tests (chosen).

## Decision
Refactor both Rust binaries into modular directory layouts:
- `janusd`:
  - thin bin entrypoint at `src/main.rs`
  - core module at `src/janusd/mod.rs`
  - extracted concerns:
    - `src/janusd/control.rs`
    - `src/janusd/adapters.rs`
    - `src/janusd/proxy.rs`
    - `src/janusd/tests.rs`
- `janus-mcp`:
  - thin bin entrypoint at `src/mcp_server.rs`
  - core module at `src/mcp/mod.rs`
  - discovery-specific module `src/mcp/discovery.rs`
- Introduce a shared protocol registry with one file per protocol under `src/protocols/`, and wire janusd + MCP discovery to consume it.

## Reasoning
- Improves maintainability and navigation by concern area.
- Keeps public behavior stable by preserving existing function logic and tests.
- Establishes a clean pattern for future protocol/adapter additions.

## Consequences
- Smaller entrypoint files with clearer runtime bootstrap.
- Shared module scope (`super::*`) is still used in some parts; future iterations can further tighten interfaces.
- File layout now matches operational domains (control API, adapters, proxy, discovery).
- Protocol metadata now has a single source of truth across janusd and MCP discovery.

## Scope
Task-specific

## Links
- Related ADR:
- Related evolution event: [20260307-175911-rust-modularization-split-monolith-files.md](../../evolution/events/20260307-175911-rust-modularization-split-monolith-files.md)
- Evidence (files/tests):
  - `src/main.rs`
  - `src/janusd/mod.rs`
  - `src/janusd/control.rs`
  - `src/janusd/adapters.rs`
  - `src/janusd/proxy.rs`
  - `src/janusd/tests.rs`
  - `src/mcp_server.rs`
  - `src/mcp/mod.rs`
  - `src/mcp/discovery.rs`
  - `src/protocols/mod.rs`
  - `src/protocols/http_proxy.rs`
  - `src/protocols/git_http.rs`
  - `src/protocols/git_ssh.rs`
  - `src/protocols/postgres_wire.rs`
  - `src/protocols/mysql_wire.rs`
  - `src/protocols/redis.rs`
  - `src/protocols/mongodb.rs`
  - `src/protocols/amqp.rs`
  - `src/protocols/kafka.rs`
  - `src/protocols/nats.rs`
  - `src/protocols/mqtt.rs`
  - `src/protocols/ldap.rs`
  - `src/protocols/sftp.rs`
  - `src/protocols/smb.rs`
  - `cargo test -q`
