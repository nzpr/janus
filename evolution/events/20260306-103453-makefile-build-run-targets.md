# Evolution Event: makefile build run targets

## Timestamp
2026-03-06T10:34:53+00:00

## Trigger
User requested Makefile targets for build and run.

## Change
- Updated root Makefile:
  - added `build` target (`cargo build --release --bin janusd --bin janus-mcp`),
  - added `run` target (`cargo run --bin janusd`),
  - changed default goal to `run`,
  - kept `start` as alias to `run`.

## Decision Link
- ADR: [0001-rust-host-daemon-secret-broker.md](../../docs/adr/0001-rust-host-daemon-secret-broker.md)
- Task decision: [TASK-20260306-makefile-build-and-run-targets.md](../../docs/decisions/TASK-20260306-makefile-build-and-run-targets.md)

## Validation Evidence
- `make -n build run`

## Outcome
Improved

## Follow-up
- None.
