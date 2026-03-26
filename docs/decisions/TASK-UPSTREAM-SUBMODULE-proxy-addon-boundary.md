# Decision: proxy addon boundary via upstream submodule

## Task
TASK-UPSTREAM-SUBMODULE

## Date
2026-03-26

## Context
The user wants upstream Codex to be updateable as a clean submodule while keeping the proxy work clearly separated as an addon and detecting upstream incompatibilities early.

## Options Considered
- Keep the current fork-only layout and manually compare with upstream.
- Introduce `openai/codex` as a submodule and add an addon manifest plus compatibility checks around the proxy-specific overlay surface.

## Decision
Add `openai/codex` as a pinned submodule under `upstream/codex`, define the proxy addon boundary in `addons/proxy/`, and enforce that boundary with a manifest-driven compatibility checker, overlay sync checks, and a rollback path to the currently published proxy baseline.

## Reasoning
The submodule gives us a clean, updateable upstream reference. The addon manifest records the managed file mapping from `addons/proxy/overlay/` into the live root tree, which files are overlays on top of upstream, and the currently published proxy rollback baseline. A compatibility checker against pinned upstream blob hashes plus overlay-sync enforcement creates fast signal when an upstream bump touches files our addon depends on or when the root tree drifts away from the addon source-of-truth, without forcing a destructive one-shot repo rewrite.

## Consequences
- Upstream Codex is now available as a pristine submodule at `upstream/codex`.
- `addons/proxy/overlay/` is the explicit source-of-truth for the addon-managed files.
- The proxy addon has an explicit compatibility and rollback surface in `addons/proxy/manifest.json`.
- CI can fail immediately when the submodule changes and an upstream overlay target no longer matches the last reviewed blob, or when the root tree drifts away from the overlay.
- The current root layout still exists during the migration, but it is now mechanically synchronized from the addon layer instead of being the conceptual source-of-truth.

## Scope
Task-specific

## Links
- Related ADR:
- Related evolution event: [20260326-000200-proxy-addon-boundary.md](../../evolution/events/20260326-000200-proxy-addon-boundary.md)
- Evidence (files/tests):
  - `.gitmodules`
  - `upstream/codex`
  - `addons/proxy/README.md`
  - `addons/proxy/overlay/`
  - `addons/proxy/manifest.json`
  - `addons/proxy/scripts/check_compat.py`
  - `addons/proxy/scripts/sync_overlay.py`
  - `addons/proxy/scripts/rollback_to_published.py`
  - `addons/proxy/scripts/update_manifest.py`
  - `.github/workflows/proxy-upstream-compat.yml`
  - `python3 addons/proxy/scripts/check_compat.py`
  - `python3 addons/proxy/scripts/sync_overlay.py --check`
