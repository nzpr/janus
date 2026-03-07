# Evolution Event: git ssh capability support

## Timestamp
2026-03-07T08:31:27+00:00

## Trigger
User requested Git proxying with SSH authentication support.

## Change
- Added `git_ssh` capability to Go and Rust Janus daemon capability registries.
- Added SSH-specific CONNECT authorization path: `git_ssh` is accepted for CONNECT on port `22`.
- Added session env wiring for `GIT_SSH_COMMAND` to tunnel SSH via Janus proxy with session token auth.
- Added session env wiring for `SSH_AUTH_SOCK` when Janus SSH agent socket is configured.
- Added Docker entrypoint flow to start `ssh-agent` and load key from Janus-only env (`JANUS_GIT_SSH_PRIVATE_KEY_FILE` / `JANUS_GIT_SSH_PRIVATE_KEY_B64` / inline).
- Updated container image runtime deps to include `openssh-client` for ssh-agent lifecycle.
- Updated docs to include capability semantics and runtime requirement (`/bin/bash`).
- Added tests for env wiring and capability-based CONNECT authorization behavior.

## Decision Link
- ADR: [0001-rust-host-daemon-secret-broker.md](../../docs/adr/0001-rust-host-daemon-secret-broker.md)
- Task decision: [TASK-20260307-git-ssh-capability.md](../../docs/decisions/TASK-20260307-git-ssh-capability.md)

## Validation Evidence
- `cd go && go test ./...`
- `cargo test -q`

## Outcome
Improved

## Follow-up
- Consider adding an integration test that runs `git ls-remote` against an SSH remote through Janus in CI.
