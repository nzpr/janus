# Decision: build and run targets

## Task
TASK-20260306-makefile

## Date
2026-03-06

## Context
User requested explicit Makefile support for build and run flows.

## Options Considered
- Keep only `start` target.
- Add `build` and `run` and keep `start` as compatibility alias (chosen).

## Decision
Add `build` and `run` targets to root Makefile, set `run` as default goal, and keep `start` delegating to `run`.

## Reasoning
- Provides the requested explicit commands.
- Preserves existing `make start` usage.
- Keeps Makefile minimal.

## Consequences
- Clearer operator workflow for local compile vs execution.
- No breaking change for existing `start` users.

## Scope
Task-specific

## Links
- Related ADR: [0001-rust-host-daemon-secret-broker.md](../adr/0001-rust-host-daemon-secret-broker.md)
- Related evolution event: [20260306-103453-makefile-build-run-targets.md](../../evolution/events/20260306-103453-makefile-build-run-targets.md)
- Evidence (files/tests):
  - `Makefile`
  - `make -n build run`
