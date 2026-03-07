# Decision: rust only remove go implementation

## Task
TASK-20260307-rust-only

## Date
2026-03-07

## Context
User requested Rust-only repository direction and asked to delete the Go implementation.
The repo previously maintained a parallel Go implementation under `go/`, which increased maintenance and duplicated behavior/docs.

## Options Considered
- Keep both Rust and Go implementations in parallel.
- Keep Go code but mark it deprecated.
- Remove `go/` from the repository and keep Rust as the single implementation (chosen).

## Decision
Delete the entire `go/` tree and keep Rust (`src/*`) as the only maintained implementation.
Update top-level README to remove the Go implementation pointer.

## Reasoning
- Matches explicit user direction.
- Reduces duplicated code paths and maintenance overhead.
- Clarifies canonical runtime and MCP behavior source.

## Consequences
- Go binaries/tests are no longer available from this repository state.
- Historical decision/evolution records still mention prior Go work for audit continuity.

## Scope
Task-specific

## Links
- Related ADR:
- Related evolution event: [20260307-174449-rust-only-remove-go-implementation.md](../../evolution/events/20260307-174449-rust-only-remove-go-implementation.md)
- Evidence (files/tests):
  - `README.md`
  - deleted `go/README.md`
  - deleted `go/cmd/janus-mcp/main.go`
  - deleted `go/cmd/janus-mcp/main_test.go`
  - deleted `go/cmd/janusd/main.go`
  - deleted `go/cmd/janusd/main_test.go`
  - deleted `go/go.mod`
  - deleted `go/go.sum`
  - `cargo test -q`
