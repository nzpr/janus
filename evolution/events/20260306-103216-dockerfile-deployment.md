# Evolution Event: dockerfile deployment

## Timestamp
2026-03-06T10:32:16+00:00

## Trigger
User requested a deployment Dockerfile.

## Change
- Added root `Dockerfile` for Rust Janus deployment (multi-stage build, runtime image with `git` + `psql`).
- Added root `.dockerignore` to reduce build context.
- Added README Docker deployment section with build/run commands and control-socket mount notes.

## Decision Link
- ADR: [0001-rust-host-daemon-secret-broker.md](../../docs/adr/0001-rust-host-daemon-secret-broker.md)
- Task decision:

## Validation Evidence
- Docker CLI not available in this environment, so image build/runtime validation could not be executed.
- File review completed for build/runtime correctness.

## Outcome
Improved

## Follow-up
- Run `docker build` and a container smoke test in an environment with Docker installed.
