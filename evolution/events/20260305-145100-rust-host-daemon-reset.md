# Evolution Event: rust host daemon reset

## Timestamp
2026-03-05T14:51:00+00:00

## Trigger
User requested a fresh restart: remove MCP-era/TS-era Janus artifacts, make Rust the implementation language, and collapse architecture decisions into the first ADR.

## Change
- Removed previous TypeScript/MCP runtime files and associated config artifacts.
- Added Rust daemon foundation (`janusd`) at `src/main.rs` with:
  - host control API over Unix socket,
  - session issuance,
  - HTTP proxying and Git HTTP path handling,
  - host-exec endpoint for allowlisted tooling.
- Switched startup flow to Rust binary via Makefile.
- Rewrote README to server-first, non-MCP operating model.
- Rebuilt ADR set so ADR-0001 is canonical for current architecture.
- Cleared old Janus decision/event records from previous architecture iterations.

## Decision Link
- ADR: [0001-rust-host-daemon-secret-broker.md](../../docs/adr/0001-rust-host-daemon-secret-broker.md)
- Task decision:

## Validation Evidence
- `cargo fmt`
- `cargo check`
- `make start`

## Outcome
Improved

## Follow-up
- Add structured audit logging and policy files.
- Add dedicated Postgres adapter instead of host-exec fallback.
- Add mTLS/TCP control endpoint option for remote host orchestration.
