# Evolution Event: rust modularization split monolith files

## Timestamp
2026-03-07T17:59:11+00:00

## Trigger
User requested modular code organization instead of single-file binaries.

## Change
- Converted Rust binaries to thin wrappers:
  - `src/main.rs` -> calls `janusd::run()`
  - `src/mcp_server.rs` -> calls `mcp::run()`
- Split janusd into modules under `src/janusd/`:
  - `mod.rs` (core shared state/helpers)
  - `control.rs` (control API routes/handlers)
  - `adapters.rs` (typed adapter handlers and command execution)
  - `proxy.rs` (proxy server and forwarding logic)
  - `tests.rs` (unit/integration tests for janusd module)
- Split MCP discovery logic into `src/mcp/discovery.rs`, with protocol/resource discovery now isolated from MCP transport loop in `src/mcp/mod.rs`.
- Added `src/protocols/` shared registry with one file per protocol (`http_proxy`, `git_http`, `git_ssh`, `postgres_wire`, `mysql_wire`, `redis`, `mongodb`, `amqp`, `kafka`, `nats`, `mqtt`, `ldap`, `sftp`, `smb`).
- Rewired janusd and MCP discovery to use shared protocol registry for capability lists, discovery metadata, and CONNECT fallback mapping.

## Decision Link
- ADR:
- Task decision: [TASK-20260307-modularization-rust-modularization-split-monolith-files.md](../../docs/decisions/TASK-20260307-modularization-rust-modularization-split-monolith-files.md)

## Validation Evidence
- `cargo test -q`

## Outcome
Improved

## Follow-up
- Continue tightening module interfaces (reduce `super::*` coupling) in future refactors.
