# Evolution Event: rust only remove go implementation

## Timestamp
2026-03-07T17:44:49+00:00

## Trigger
User requested Rust-only direction and asked to delete the Go implementation.

## Change
- Removed the entire `go/` directory from version control:
  - `go/README.md`
  - `go/cmd/janus-mcp/main.go`
  - `go/cmd/janus-mcp/main_test.go`
  - `go/cmd/janusd/main.go`
  - `go/cmd/janusd/main_test.go`
  - `go/go.mod`
  - `go/go.sum`
- Updated root `README.md` to remove the Go reimplementation pointer.

## Decision Link
- ADR:
- Task decision: [TASK-20260307-rust-only-rust-only-remove-go-implementation.md](../../docs/decisions/TASK-20260307-rust-only-rust-only-remove-go-implementation.md)

## Validation Evidence
- `cargo test -q`

## Outcome
Improved

## Follow-up
- If needed later, reintroduce a second implementation in a separate repository to avoid in-repo duplication.
