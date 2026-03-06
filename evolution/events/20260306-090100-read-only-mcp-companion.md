# Evolution Event: read only mcp companion

## Timestamp
2026-03-06T09:01:00+00:00

## Trigger
User requested MCP support so sandboxed LLM can discover Janus capabilities without secret exposure.

## Change
- Added `janus-mcp` binary (`src/mcp_server.rs`) implementing stdio MCP JSON-RPC framing.
- Implemented MCP methods: `initialize`, `ping`, `tools/list`, `tools/call`, `resources/list`, `prompts/list`.
- Added safe read-only tools:
  - `janus.health`
  - `janus.capabilities`
  - `janus.safety`
- MCP tool surface intentionally excludes session creation, token issuance, secret reads, and control-socket path disclosure.
- Added README section with concrete MCP config JSON.
- Updated ADR-0001 to allow optional read-only MCP companion under strict constraints.

## Decision Link
- ADR: [0001-rust-host-daemon-secret-broker.md](../../docs/adr/0001-rust-host-daemon-secret-broker.md)
- Task decision:

## Validation Evidence
- `cargo fmt`
- `cargo check`
- `cargo test`

## Outcome
Improved

## Follow-up
- Add end-to-end MCP integration test against running `janusd`.
