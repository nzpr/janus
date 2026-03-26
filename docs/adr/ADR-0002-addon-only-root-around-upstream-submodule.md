# ADR-0002: Use an addon-only root around the upstream Codex submodule

## Status
Accepted

## Date
2026-03-26

## Context
The repository originally still looked and behaved like a full Codex fork even after introducing `upstream/codex` as a pinned submodule. That left two copies of the Codex tree in the repo, caused unrelated upstream workflows to trigger on every push, and made the submodule boundary hard to understand.

## Decision
Make the repository root addon-only. Keep upstream Codex solely in `upstream/codex`, keep proxy-specific overrides in `addons/proxy/overlay`, and build releases from a temporary materialized workspace exported from the submodule with the overlay applied.

## Options Considered
- Keep the incremental fork layout with a duplicated live Codex tree in the root.
- Remove the duplicated root tree and treat the repo as addon infrastructure around the upstream submodule.

## Consequences
### Positive
- The repository clearly separates upstream source from addon-owned changes.
- Unrelated upstream workflows and repo scaffolding are no longer part of the root repo.
- Release automation has an explicit, reproducible build input: `upstream/codex` plus the overlay.

### Negative
- Local development now requires either inspecting the submodule directly or materializing a temporary workspace.
- Release scripts and compatibility checks must understand both repo-owned overlay files and workspace overlay files.

## References
- Related task(s): TASK-UPSTREAM-SUBMODULE
- Related decision notes:
  - [TASK-UPSTREAM-SUBMODULE-proxy-addon-boundary.md](../decisions/TASK-UPSTREAM-SUBMODULE-proxy-addon-boundary.md)
- Related evolution events:
  - [20260326-000200-proxy-addon-boundary.md](../../evolution/events/20260326-000200-proxy-addon-boundary.md)
- Source links:
  - `upstream/codex`
  - `addons/proxy/overlay`
  - `addons/proxy/scripts/materialize_workspace.py`
  - `.github/workflows/proxy-release.yml`
