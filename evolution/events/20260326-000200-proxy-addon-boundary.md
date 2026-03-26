# Evolution Event: proxy addon boundary

## Timestamp
2026-03-26T00:02:00+00:00

## Trigger
User asked to reorganize the repository so Codex could be updated as a submodule and the proxy would be maintained as a clearly separated addon with incompatibility detection.

## Change
Added `openai/codex` as the `upstream/codex` submodule, created `addons/proxy/` with an overlay manifest and compatibility/update scripts, and added a dedicated `proxy-upstream-compat` workflow that checks the proxy overlay assumptions against the pinned upstream submodule.

## Decision Link
- ADR:
- Task decision: [TASK-UPSTREAM-SUBMODULE-proxy-addon-boundary.md](../../docs/decisions/TASK-UPSTREAM-SUBMODULE-proxy-addon-boundary.md)

## Validation Evidence
- `.gitmodules`
- `upstream/codex`
- `addons/proxy/manifest.json`
- `addons/proxy/scripts/check_compat.py`
- `.github/workflows/proxy-upstream-compat.yml`
- `python3 addons/proxy/scripts/check_compat.py`
- `python3 -c "import pathlib, yaml; yaml.safe_load(pathlib.Path('.github/workflows/proxy-upstream-compat.yml').read_text())"`

## Outcome
Success

## Follow-up
- Move additional live proxy files under the addon source-of-truth over time instead of keeping the current root overlay layout indefinitely.
- Run the compatibility checker whenever the submodule is bumped to a newer upstream commit.
