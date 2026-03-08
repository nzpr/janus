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

## Decision Link
- ADR:
- Task decision: [TASK-20260307-readme-ux-readme-user-focused-startup-and-mcp.md](../../docs/decisions/TASK-20260307-readme-ux-readme-user-focused-startup-and-mcp.md)

## Validation Evidence
- Manual README review

## Outcome
Improved

## Follow-up
- If needed, add separate developer/internal API docs outside root README.
