# Evolution Event: readme user focused startup and mcp

## Timestamp
2026-03-07T18:31:11+00:00

## Trigger
User requested README to focus on starting server + MCP only, not low-level proxied call mechanics.

## Change
- Reworked `README.md` into operator-focused flow:
  - Start Janusd (host/docker)
  - Start MCP companion
  - MCP behavior summary
- Removed detailed manual control/proxy API call walkthroughs from root README.
- Retained environment variable and safety sections relevant to operators.
- Added explicit "Why Session Tokens" section explaining delegated scoped access and blast-radius reduction.
- Reordered README to start with architecture and trust boundaries.
- Added architecture diagram and interfaces/ports table.
- Added detailed step-by-step runbook for host startup, jailed MCP configuration, and session injection responsibilities.
- Added complete protocol capability matrix (all current proxyable protocols and typical target ports).
- Split deployment guidance into exact Host and Docker recipes for easier operator execution.
- Added top-level quickstart block with copy-paste steps for host start, jailed MCP start, session issuance, and env injection.
- Reworked quickstart to use a single `.env` file with all protocol capabilities enabled.
- Added explicit note for why `JANUS_DISCOVERY_BIND` is required in jailed MCP mode.
- Removed repository URL line from README header to keep startup docs focused.
- Rephrased startup guidance sentence to remove confusing proxy-call wording.
- Added explicit supported-protocol list near top of README.
- Shifted quickstart to `.env.example` -> `.env` workflow with explicit secret/capability edits.
- Updated `.env.example` to enable all protocol capabilities by default and include protocol/port reference comments.
- Expanded README guidance for configuring non-HTTP protocols via capability + `JANUS_ALLOWED_HOSTS`.
- Rewrote README into a clearer operator-first structure with:
  - component trust-boundary table,
  - per-protocol usability matrix,
  - strict jailed quickstart,
  - explicit sidecar usage patterns (`janus-pg-sidecar`, `janus-tunnel`),
  - compact security checklist.
- Performed additional cleanup pass to make README shorter and easier to scan:
  - removed redundant explanatory sections,
  - kept a single fast-start workflow,
  - retained protocol readiness matrix and sidecar requirements.

## Decision Link
- ADR:
- Task decision: [TASK-20260307-readme-ux-readme-user-focused-startup-and-mcp.md](../../docs/decisions/TASK-20260307-readme-ux-readme-user-focused-startup-and-mcp.md)

## Validation Evidence
- Manual README review

## Outcome
Improved

## Follow-up
- If needed, add separate developer/internal API docs outside root README.
