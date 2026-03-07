# Decision: add git ssh capability via proxy CONNECT tunneling

## Task
TASK-20260307-git-ssh-capability

## Date
2026-03-07

## Context
Users needed Git over SSH support while keeping Janus session-token capability checks and host allowlist enforcement.

## Options Considered
- Keep `git_http` only and reject SSH-based remotes.
- Add a broad SSH adapter endpoint for arbitrary host shell execution.
- Add a dedicated `git_ssh` capability that wires `GIT_SSH_COMMAND` through Janus proxy CONNECT (chosen).

## Decision
Introduce `git_ssh` as a first-class capability in both Go and Rust daemons. Session env now includes `GIT_SSH_COMMAND` that tunnels SSH through Janus CONNECT with token auth.

## Reasoning
- Preserves Janus security model (short-lived token + per-session allowlist).
- Enables SSH key-based Git workflows without exposing upstream credentials.
- Avoids adding generic execution surfaces.

## Consequences
- Runtime needs `/bin/bash` for injected ProxyCommand script.
- `git_ssh` scope is intentionally narrow: CONNECT authorization is limited to port `22`.

## Scope
Task-specific

## Links
- Related ADR: [0001-rust-host-daemon-secret-broker.md](../adr/0001-rust-host-daemon-secret-broker.md)
- Related evolution event: [20260307-083127-git-ssh-capability.md](../../evolution/events/20260307-083127-git-ssh-capability.md)
- Evidence (files/tests):
  - `go/cmd/janusd/main.go`
  - `go/cmd/janusd/main_test.go`
  - `src/main.rs`
  - `README.md`
  - `go/README.md`
  - `cd go && go test ./...`
  - `cargo test -q`
