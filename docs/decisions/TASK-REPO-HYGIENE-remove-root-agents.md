# Decision: remove root agents file

## Task
TASK-REPO-HYGIENE

## Date
2026-03-26

## Context
The root `AGENTS.md` was no longer wanted in the repository and had become the only remaining tracked dirty file blocking a clean push of the accumulated proxy and Pages work.

## Options Considered
- Keep the root `AGENTS.md` in the repository.
- Remove the root `AGENTS.md` and push the current `main` history cleanly.

## Decision
Remove the root `AGENTS.md` and push the current `main` branch without it.

## Reasoning
The user explicitly asked to remove the file. Deleting it keeps the repository root clean and avoids carrying a stale tracked instruction file forward with the proxy work.

## Consequences
- The repository no longer carries a root-level `AGENTS.md`.
- The current `main` history can be pushed cleanly.

## Scope
Task-specific

## Links
- Related ADR:
- Related evolution event: [20260326-000100-remove-root-agents.md](../../evolution/events/20260326-000100-remove-root-agents.md)
- Evidence (files/tests):
  - `AGENTS.md`
  - `git status -sb`
