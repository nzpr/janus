# Evolution Event: connect port capability iteration

## Timestamp
2026-03-07T16:28:29+00:00

## Trigger
User requested to proceed from planning into concrete protocol iteration.

## Change
- Added new proxy capability constants in Go and Rust for planned non-HTTP protocols.
- Added CONNECT port-to-capability authorization map for protocol-scoped access.
- Extended config metadata (`supports.proxy`) and docs to include protocol capabilities.
- Added unit tests in both implementations for protocol capability CONNECT authorization behavior.

## Decision Link
- ADR:
- Task decision: [TASK-20260307-protocol-iteration-connect-port-capability-iteration.md](../../docs/decisions/TASK-20260307-protocol-iteration-connect-port-capability-iteration.md)

## Validation Evidence
- `cd go && go test ./...`
- `cargo test -q`

## Outcome
Improved

## Follow-up
- Add protocol-level guardrails (timeouts/limits) per capability beyond port-level checks.
- Implement deeper protocol-aware adapters where port-level capability is insufficient.
