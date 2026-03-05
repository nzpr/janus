# Evolution Event: host side http rewrite broker bootstrap

## Timestamp
2026-03-05T10:49:26+00:00

## Trigger
Need to separate secret-broker protocol rewrite behavior from Jim into its own reusable project with full Jim contract context.

## Change
- Created standalone `janus` project scaffold (`package.json`, `tsconfig.json`, `Makefile`, README).
- Implemented broker CLI in `src/janus.ts` with:
  - host-env grant resolution,
  - Git remote host discovery,
  - HTTP auth header injection proxy adapters,
  - non-interactive Git rewrite env generation,
  - command execution mode with injected rewrite env.
- Initialized Jim contract in the new project so AGENTS, playbooks, and audit scripts are present.
- Added project-local bootstrap decision/event records.

## Decision Link
- ADR:
- Task decision: [TASK-001-host-side-http-rewrite-broker-bootstrap.md](../../docs/decisions/TASK-001-host-side-http-rewrite-broker-bootstrap.md)

## Validation Evidence
- `bun run src/janus.ts help`
- `bun run src/janus.ts plan`

## Outcome
Improved

## Follow-up
- Add per-host credential mapping support when different hosts require different usernames/passwords.
