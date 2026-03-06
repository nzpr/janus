# Evolution Event: strict capability adapters hardening

## Timestamp
2026-03-06T08:45:58+00:00

## Trigger
User requested completion of strict host-broker model: capability APIs only and no raw secret exposure to sandboxed agents.

## Change
- Reworked daemon runtime around explicit capabilities.
- Removed generic host execution API (`/v1/exec`).
- Removed control socket path from session env output.
- Added typed control APIs:
  - `POST /v1/postgres/query`
  - `POST /v1/deploy/kubectl`
  - `POST /v1/deploy/helm`
  - `POST /v1/deploy/terraform`
- Enforced capability checks on proxy and adapter paths.
- Added adapter argument policy checks and forbidden credential flags for deployment tools.
- Added output redaction for known host secret values.
- Added startup/help text updates for strict model.
- Added unit tests for capability normalization, env shaping, host matching, argument validation, and redaction.

## Decision Link
- ADR: [0001-rust-host-daemon-secret-broker.md](../../docs/adr/0001-rust-host-daemon-secret-broker.md)
- Task decision:

## Validation Evidence
- `cargo fmt`
- `cargo check`
- `cargo test` (7 passed)

## Outcome
Improved

## Follow-up
- Add persistent audit log sink (file/syslog/OTel) for production deployments.
- Add protocol-specific adapters beyond Postgres/deploy tooling as needed.
