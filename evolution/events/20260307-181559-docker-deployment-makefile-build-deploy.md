# Evolution Event: docker deployment makefile build deploy

## Timestamp
2026-03-07T18:15:59+00:00

## Trigger
User requested Docker deployment and Makefile build/deploy workflow.

## Change
- Expanded `Makefile` with Docker lifecycle targets:
  - `docker-build`
  - `deploy` (build+run with configurable vars and env-file support)
  - `stop`
  - `logs`
- Updated `health` target to automatically resolve Docker vs local control socket.
- Added `.env.docker.example` template for container env configuration.
- Updated README Docker section with Makefile-based deployment steps.

## Decision Link
- ADR:
- Task decision: [TASK-20260307-docker-deploy-docker-deployment-makefile-build-deploy.md](../../docs/decisions/TASK-20260307-docker-deploy-docker-deployment-makefile-build-deploy.md)

## Validation Evidence
- `make -n deploy`
- `make -n health`
- `cargo test -q`

## Outcome
Improved

## Follow-up
- Optionally add a compose profile if multi-service deployment (db/k8s tooling sidecars) is later required.
