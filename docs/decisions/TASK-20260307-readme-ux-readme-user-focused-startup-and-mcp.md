# Decision: readme user focused startup and mcp

## Task
TASK-20260307-readme-ux

## Date
2026-03-07

## Context
User asked for README to be useful to operators only: how to start server and MCP, not how to make proxied/control calls.
Previous README contained detailed low-level API call examples that are LLM-internal operational details.
Follow-up question asked why session tokens exist if LLM can still use them, requiring explicit threat-model explanation in README.
Additional follow-up requested a more detailed README with architecture first, diagram, and exact operational runbook.
Latest follow-up requested crystal-clear deployment/usage instructions and explicit list of every supported proxy protocol.

## Options Considered
- Keep detailed proxy/control API examples in root README.
- Move low-level call details to separate docs and keep root README startup-focused (chosen).

## Decision
Rewrite root README flow to user-focused startup:
- emphasize only two steps: start `janusd`, start `janus-mcp`.
- keep host and docker startup paths (`make start`, `make deploy`).
- keep MCP configuration and discovery behavior summary.
- remove manual control API/proxy usage walkthroughs from root README.
- add explicit "Why Session Tokens" section in safety model clarifying:
  - tokens are for scoped, revocable delegated access,
  - tokens limit blast radius versus exposing upstream secrets,
  - tokens do not imply trust in LLM process behavior.
- restructure README order and depth:
  - architecture section first,
  - explicit architecture chart,
  - interface/port table,
  - exact step-by-step operator runbook for jailed LLM deployment.
- add explicit protocol catalog table listing all currently supported proxy capabilities and typical ports.
- split deployment into concrete recipes (host process and docker) with exact command sequences.

## Reasoning
- Aligns docs with operator mental model.
- Reduces noise and potential misuse.
- Keeps protocol calling semantics where they belong: MCP + agent behavior.

## Consequences
- Root README is shorter and task-focused for end users.
- Advanced internal API usage may require separate internal docs if needed later.
- Operators get explicit rationale for session-token design and expected security properties.
- Operators now get a concrete sequence for host startup, jailed MCP wiring, and responsibilities split (host supervisor vs jailed LLM).
- Operators can directly map required upstream protocol to a concrete Janus capability.

## Scope
Task-specific

## Links
- Related ADR:
- Related evolution event: [20260307-183111-readme-user-focused-startup-and-mcp.md](../../evolution/events/20260307-183111-readme-user-focused-startup-and-mcp.md)
- Evidence (files/tests):
  - `README.md`
