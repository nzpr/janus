# Evolution Event: remove root agents

## Timestamp
2026-03-26T00:01:00+00:00

## Trigger
User asked to remove the root `AGENTS.md` and push the current repository state.

## Change
Deleted the root `AGENTS.md`, recorded the cleanup decision, and prepared the repository for a clean push of the accumulated proxy and Pages changes.

## Decision Link
- ADR:
- Task decision: [TASK-REPO-HYGIENE-remove-root-agents.md](../../docs/decisions/TASK-REPO-HYGIENE-remove-root-agents.md)

## Validation Evidence
- `AGENTS.md`
- `git status -sb`

## Outcome
Success

## Follow-up
- Push `main` so the Pages workflow and the latest proxy changes land on the remote branch.
