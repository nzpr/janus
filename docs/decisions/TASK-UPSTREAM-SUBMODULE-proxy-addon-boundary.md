# Decision: addon-only root around upstream codex submodule

## Task
TASK-UPSTREAM-SUBMODULE

## Date
2026-03-26

## Context
The user wants upstream Codex to be updateable as a clean submodule while keeping the proxy work clearly separated as an addon and detecting upstream incompatibilities early.

## Options Considered
- Keep the current fork-only layout and manually compare with upstream.
- Introduce `openai/codex` as a submodule and stop there with a duplicated live root tree.
- Introduce `openai/codex` as a submodule and complete the migration so the root repo only contains addon-owned files while releases build from a materialized upstream workspace plus overlay.

## Decision
Use `upstream/codex` as the only checked-in copy of upstream Codex, keep proxy overrides in `addons/proxy/overlay`, keep only addon-owned files in the repo root, and build releases from a temporary materialized workspace exported from the submodule with the overlay applied. Keep manifest-driven compatibility checks, repo-overlay sync checks, and the rollback path to the currently published proxy baseline.

## Reasoning
The submodule gives us a clean, updateable upstream reference. Completing the migration removes the confusing double-tree layout and makes the root repository clearly about the proxy addon itself rather than a second Codex checkout. The manifest now distinguishes repo-owned overlay files from workspace overlay files so CI can verify the root repo surface, validate reviewed upstream blob hashes, and build from a temporary exported workspace without reintroducing a duplicated root tree.

## Consequences
- Upstream Codex is now available as a pristine submodule at `upstream/codex`.
- The repository root no longer contains a second checked-in `codex-cli`, `codex-rs`, `sdk`, or upstream docs tree.
- `addons/proxy/overlay/` is the explicit source-of-truth for the addon-managed files.
- The proxy addon has an explicit compatibility and rollback surface in `addons/proxy/manifest.json`.
- CI can fail immediately when the submodule changes and an upstream overlay target no longer matches the last reviewed blob, or when the repo-owned root files drift away from the overlay.
- Release automation must materialize a temporary workspace from `upstream/codex` plus the overlay before building.

## Scope
Task-specific

## Links
- Related ADR:
  - [ADR-0002-addon-only-root-around-upstream-submodule.md](../adr/ADR-0002-addon-only-root-around-upstream-submodule.md)
- Related evolution event: [20260326-000200-proxy-addon-boundary.md](../../evolution/events/20260326-000200-proxy-addon-boundary.md)
- Evidence (files/tests):
  - `.gitmodules`
  - `upstream/codex`
  - `addons/proxy/README.md`
  - `addons/proxy/overlay/`
  - `addons/proxy/manifest.json`
  - `addons/proxy/scripts/check_compat.py`
  - `addons/proxy/scripts/materialize_workspace.py`
  - `addons/proxy/scripts/sync_overlay.py`
  - `addons/proxy/scripts/rollback_to_published.py`
  - `addons/proxy/scripts/update_manifest.py`
  - `.github/workflows/proxy-upstream-compat.yml`
  - `.github/workflows/proxy-release.yml`
  - `python3 addons/proxy/scripts/check_compat.py`
  - `python3 addons/proxy/scripts/sync_overlay.py --check`
