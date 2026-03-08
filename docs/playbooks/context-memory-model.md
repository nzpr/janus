# Jim Context and Memory Model

This document defines how Jim maintains continuity without requiring manual session management.

## Context Layers

Jim uses multiple context layers with different durability:

1. Runtime conversation context (Codex session)
- Primary working context while a session is active.
- Resumed automatically via `.codex_session` when available.

2. Short-term handoff context (automatic, local cache)
- Jim writes a handoff snapshot automatically at session stop:
  - `.jim/.cache/handoff/latest.md`
  - `.jim/.cache/handoff/<timestamp>.md`
- Snapshot includes:
  - detected completion signal status,
  - changed files,
  - output tail for fast continuity.
- This is local runtime cache, not project source-of-truth.

3. Project memory context (durable project docs)
- On first run, Jim can bootstrap:
  - `docs/project-memory/project-overview.md`
  - `docs/project-memory/current-priorities.md`
  - `docs/project-memory/working-agreements.md`
- These are editable by humans and agents and survive sessions.

4. Process and decision context (auditable history)
- Architectural and task decisions:
  - `docs/adr/`
  - `docs/decisions/`
- Evolution evidence:
  - `evolution/events/`
- This is authoritative long-term process memory.

## Project Memory vs Decision Records

- `docs/project-memory/*` is operational context for current work.
- `docs/adr/*`, `docs/decisions/*`, and `evolution/events/*` are decision history and evidence.
- Do not duplicate decision logs in project-memory files; link to the relevant decision/event instead.
- Keep project-memory concise and periodically prune stale entries.

5. Optional indexed retrieval context
- If storage/indexer are available, Jim syncs non-code knowledge into vector-backed memory.
- If unavailable, Jim continues with markdown fallback.
- Indexed memory is an optimization, not a hard dependency.

## Completion Semantics

Task completion is semantic, not "container exited":

- Jim detects completion signals from session output.
- Only then it runs Done Gate and optional publish prompts.
- Manual fallback remains available via:
  - `bun jim.ts complete`
  - `bun jim.ts done`

## Git and Cache Policy

- `.codex_session` is gitignored workspace runtime state.
- `.jim/` is gitignored Jim runtime state (including handoff cache).
- In Jim-managed target repos, process/governance internals are gitignored by default:
  - `AGENTS.md`, `docs/adr/`, `docs/decisions/`, `docs/playbooks/`, `evolution/`.

## Design Intent

- Seamless continuity by default.
- No manual choreography required for normal use.
- Clear split between:
  - ephemeral runtime cache,
  - durable project knowledge,
  - durable process governance.
