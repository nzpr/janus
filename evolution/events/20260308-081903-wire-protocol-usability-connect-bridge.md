# Evolution Event: wire protocol usability connect bridge

## Timestamp
2026-03-08T08:19:03+00:00

## Trigger
User identified that non-HTTP protocols were listed but not truly usable end-to-end with standard clients like `psql`.

## Change
- Added new `janus-tunnel` binary to provide local TCP -> Janus CONNECT bridging.
- Added protocol default-port resolution (`--protocol`) and target-host/port routing in `janus-tunnel`.
- Added proxy auth sourcing from session env (`JANUS_CONNECT_PROXY_URL`, fallback `ALL_PROXY`/`HTTP_PROXY`).
- Updated session env generation to emit `JANUS_CONNECT_PROXY_URL` for any CONNECT-capable protocol capability.
- Added tests for tunnel URL parsing/protocol mapping and session env connect-proxy exposure.
- Updated README and `.env.example` to document usable wire-protocol flow and configuration.
- Updated build/deploy surfaces (`Cargo.toml`, `Makefile`, `Dockerfile`) to include `janus-tunnel`.

## Decision Link
- ADR:
- Task decision: [TASK-20260308-wire-protocol-usability-connect-bridge.md](../../docs/decisions/TASK-20260308-wire-protocol-usability-connect-bridge.md)

## Validation Evidence
- `cargo test -q`

## Outcome
Improved

## Follow-up
- Add optional convenience wrappers (`janus-pg`, `janus-redis`) if operators want one-command workflows per protocol.
