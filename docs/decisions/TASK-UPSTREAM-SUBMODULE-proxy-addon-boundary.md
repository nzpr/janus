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
Add `openai/codex` as a pinned submodule under `upstream/codex`, define the proxy addon boundary in `addons/proxy/`, and enforce that boundary with a manifest-driven compatibility checker and CI workflow.

## Reasoning
The submodule gives us a clean, updateable upstream reference. The addon manifest records which live files are overlays on top of upstream and which are fully first-party. A compatibility checker against pinned upstream blob hashes creates fast signal when an upstream bump touches files our addon depends on, without forcing a destructive one-shot repo rewrite.

## Consequences
- Upstream Codex is now available as a pristine submodule at `upstream/codex`.
- The proxy addon has an explicit compatibility surface in `addons/proxy/manifest.json`.
- CI can fail immediately when the submodule changes and an upstream overlay target no longer matches the last reviewed blob.
- The current root layout still exists during the migration, so this is a boundary-establishing step rather than a full physical extraction.

## Scope
Task-specific

## Links
- Related ADR:
- Related evolution event: [20260326-000200-proxy-addon-boundary.md](../../evolution/events/20260326-000200-proxy-addon-boundary.md)
- Evidence (files/tests):
  - `.gitmodules`
  - `upstream/codex`
  - `addons/proxy/README.md`
  - `addons/proxy/manifest.json`
  - `addons/proxy/scripts/check_compat.py`
  - `addons/proxy/scripts/update_manifest.py`
  - `.github/workflows/proxy-upstream-compat.yml`
  - `python3 addons/proxy/scripts/check_compat.py`
