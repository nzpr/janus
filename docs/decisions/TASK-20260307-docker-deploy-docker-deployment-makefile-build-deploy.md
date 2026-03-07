# Decision: docker deployment makefile build deploy

## Task
TASK-20260307-docker-deploy

## Date
2026-03-07

## Context
User requested Docker deployment support plus Makefile build/deploy workflow.
Repository already had a Dockerfile and minimal Makefile, but no first-class deploy lifecycle targets.

## Options Considered
- Keep docs-only docker commands and leave Makefile minimal.
- Add deployment shell scripts and keep Makefile as thin wrappers.
- Expand Makefile with explicit Docker lifecycle targets and deployment variables, plus env template and README updates (chosen).

## Decision
Implement deployment workflow in Makefile:
- keep `build` for Rust binaries.
- add `docker-build`, `deploy`, `stop`, `logs`.
- enhance `health` to auto-select docker socket path (`/tmp/janus/janusd-control.sock`) when present, otherwise local default.

Add container env template:
- `.env.docker.example` for `make deploy` (`JANUS_ENV_FILE`, default `.env`).

Document usage in README Docker section using Make targets.

## Reasoning
- Provides one-command build/deploy ergonomics.
- Keeps local-run and docker-run flows both supported.
- Avoids extra tooling dependency (compose) while still providing configurable deployment parameters.

## Consequences
- Deployment behavior is now standardized through Make variables:
  `IMAGE`, `CONTAINER`, `PROXY_PORT`, `SOCKET_DIR`, `JANUS_ENV_FILE`.
- Operators should provide real secrets via `.env`/env file before `make deploy`.

## Scope
Task-specific

## Links
- Related ADR:
- Related evolution event: [20260307-181559-docker-deployment-makefile-build-deploy.md](../../evolution/events/20260307-181559-docker-deployment-makefile-build-deploy.md)
- Evidence (files/tests):
  - `Makefile`
  - `.env.docker.example`
  - `README.md`
  - `make -n deploy`
  - `make -n health`
  - `cargo test -q`
