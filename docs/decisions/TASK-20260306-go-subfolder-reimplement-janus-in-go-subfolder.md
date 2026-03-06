# Decision: reimplement janus in go subfolder

## Task
TASK-20260306-go-subfolder

## Date
2026-03-06

## Context
User requested a Go reimplementation while preserving the strict host-broker security model and existing Rust implementation history.

## Options Considered
- Keep Rust-only implementation.
- Replace root implementation with Go immediately.
- Add a full Go reimplementation in `go/` subfolder while keeping Rust implementation intact (chosen).

## Decision
Implement `janusd` and `janus-mcp` in Go under `go/` as a parallel implementation, without deleting Rust code.

## Reasoning
- Meets the explicit language requirement quickly.
- Preserves working Rust baseline and docs/history.
- Allows side-by-side validation and migration later without hard cutover risk.

## Consequences
- Two implementation tracks now exist and need parity management.
- Migration decision (Go becoming primary) can be made later with lower risk.

## Scope
Task-specific

## Links
- Related ADR: [0001-rust-host-daemon-secret-broker.md](../adr/0001-rust-host-daemon-secret-broker.md)
- Related evolution event: [20260306-101458-go-subfolder-reimplementation.md](../../evolution/events/20260306-101458-go-subfolder-reimplementation.md)
- Evidence (files/tests):
  - `go/cmd/janusd/main.go`
  - `go/cmd/janus-mcp/main.go`
  - `go/cmd/janusd/main_test.go`
  - `go/cmd/janus-mcp/main_test.go`
  - `cd go && go test ./...`
