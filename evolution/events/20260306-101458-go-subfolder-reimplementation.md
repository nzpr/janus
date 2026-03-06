# Evolution Event: go subfolder reimplementation

## Timestamp
2026-03-06T10:14:58+00:00

## Trigger
User requested full reimplementation in Go under a dedicated `go/` subfolder.

## Change
- Added Go module at `go/` with two binaries:
  - `cmd/janusd`: strict host-broker daemon (proxy, capability sessions, typed adapters, redaction).
  - `cmd/janus-mcp`: read-only MCP companion using stdio framing and safe metadata tools.
- Added Go tests for critical safety/capability behavior.
- Added `go/README.md` and root README pointer.

## Decision Link
- ADR: [0001-rust-host-daemon-secret-broker.md](../../docs/adr/0001-rust-host-daemon-secret-broker.md)
- Task decision: [TASK-20260306-go-subfolder-reimplement-janus-in-go-subfolder.md](../../docs/decisions/TASK-20260306-go-subfolder-reimplement-janus-in-go-subfolder.md)

## Validation Evidence
- `cd go && go test ./...`
- `cd go && go build ./...`

## Outcome
Improved

## Follow-up
- Decide whether Go implementation should become primary and if/when Rust implementation is deprecated.
