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
Latest follow-up requested an explicit top-level quickstart with copy-paste steps for running with a sandboxed agent.
Latest follow-up requested replacing export-based quickstart with an explicit `.env` all-protocol configuration and removing repository metadata noise.
Latest follow-up requested clearer wording, explicit protocol support near top, and `.env.example`-driven setup guidance.
Latest follow-up requested `.env.example` itself to show how non-HTTP protocols are configured, not only HTTP/Git-oriented defaults.

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
- add top-level quickstart section with exact host start, jailed MCP start, session issuance example, and env injection guidance.
- move quickstart to `.env`-driven configuration showing all capabilities enabled in one place.
- explicitly explain why `JANUS_DISCOVERY_BIND` is needed for jailed MCP.
- remove non-essential repository metadata line from README header.
- rephrase startup sentence to avoid proxy-internals jargon.
- add explicit supported-protocol list near README top.
- direct operators to configure secrets in `.env` using `.env.example` as base.
- make `.env.example` all-protocol by default and add protocol-to-port reference comments.
- clarify in README that non-HTTP protocols are configured via capability + allowed hosts.

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
- Operators can get started immediately from one compact command sequence before reading full architecture details.
- Operators now have a concrete all-protocol `.env` template instead of piecemeal exports.
- Operators get clearer first-read instructions without ambiguous wording.
- Operators can now configure non-HTTP protocols directly from `.env.example` without guessing extra knobs.

## Scope
Task-specific

## Links
- Related ADR:
- Related evolution event: [20260307-183111-readme-user-focused-startup-and-mcp.md](../../evolution/events/20260307-183111-readme-user-focused-startup-and-mcp.md)
- Evidence (files/tests):
  - `README.md`
