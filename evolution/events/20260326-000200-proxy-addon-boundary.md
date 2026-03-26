# Evolution Event: proxy addon boundary

## Timestamp
2026-03-26T00:02:00+00:00

## Trigger
User asked to reorganize the repository so Codex could be updated as a submodule and the proxy would be maintained as a clearly separated addon with incompatibility detection.

## Change
Added `openai/codex` as the `upstream/codex` submodule, completed the migration to an addon-only repo root, kept the proxy overrides under `addons/proxy/overlay/`, added manifest-driven sync/compatibility/materialization/update/rollback scripts, removed the duplicated upstream Codex tree and unrelated upstream CI from the root repo, and wired release builds to materialize a temporary upstream workspace with the overlay applied.

## Decision Link
- ADR: [ADR-0002-addon-only-root-around-upstream-submodule.md](../../docs/adr/ADR-0002-addon-only-root-around-upstream-submodule.md)
- Task decision: [TASK-UPSTREAM-SUBMODULE-proxy-addon-boundary.md](../../docs/decisions/TASK-UPSTREAM-SUBMODULE-proxy-addon-boundary.md)

## Validation Evidence
- `.gitmodules`
- `upstream/codex`
- `addons/proxy/manifest.json`
- `addons/proxy/scripts/check_compat.py`
- `addons/proxy/scripts/materialize_workspace.py`
- `addons/proxy/scripts/sync_overlay.py`
- `addons/proxy/scripts/rollback_to_published.py`
- `.github/workflows/proxy-upstream-compat.yml`
- `.github/workflows/proxy-release.yml`
- `python3 addons/proxy/scripts/check_compat.py`
- `python3 addons/proxy/scripts/sync_overlay.py --check`
- `python3 addons/proxy/scripts/materialize_workspace.py --dest <tempdir>`
- rollback rehearsal against published baseline `75c7f851815637e74169d4162b32630ee172e631`

## Outcome
Success

## Follow-up
- Run the compatibility checker whenever the submodule is bumped to a newer upstream commit.
