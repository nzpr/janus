# Proxy Addon

This directory is the first-party layer that sits on top of the upstream Codex submodule in [`upstream/codex`](../../upstream/codex).

## Final Model

- `upstream/codex` is the only copy of the upstream Codex source in this repository.
- The repository root holds addon-owned repo files only: proxy workflows, docs, site assets, decision records, and the addon scripts.
- [`overlay/`](./overlay/) contains the files that are applied on top of a materialized copy of `upstream/codex` for build and release work.
- [`manifest.json`](./manifest.json) records:
  - the pinned upstream commit
  - the currently published rollback baseline
  - which overlay files sync back into the repo root
  - which overlay files patch the materialized upstream workspace
  - the upstream blob revisions that were last reviewed

## Materialized Workspace

To create a buildable temporary workspace from upstream plus the overlay:

```sh
python3 addons/proxy/scripts/materialize_workspace.py --dest /tmp/janus-workspace
```

That exports `upstream/codex` into the destination and then copies every overlay file into its target path inside that temporary tree.

## Repo-Owned Overlay Files

These files are source-controlled in [`overlay/`](./overlay/) and synced into the repo root with:

```sh
python3 addons/proxy/scripts/sync_overlay.py
```

Current repo-owned overlay targets include:

- `.github/workflows/proxy-pages.yml`
- `.github/workflows/proxy-release.yml`
- `.github/workflows/proxy-upstream-compat.yml`
- `README-proxy.md`
- `site/`

## Upstream Overlay Surface

The upstream-backed proxy overlay currently covers:

- `codex-rs/Cargo.lock`
- `codex-rs/responses-api-proxy/`

Those paths no longer live as a second checked-in tree at the repo root. They exist only in:

- the pristine submodule at [`upstream/codex`](../../upstream/codex)
- the addon overlay at [`overlay/`](./overlay/)
- temporary materialized workspaces created for build or inspection

## Update Flow

1. Update the submodule:
   ```sh
   git submodule update --remote upstream/codex
   ```
2. Re-check compatibility:
   ```sh
   python3 addons/proxy/scripts/check_compat.py
   ```
3. If the new upstream files are acceptable, refresh the reviewed blob hashes:
   ```sh
   python3 addons/proxy/scripts/update_manifest.py
   ```
4. If you changed repo-owned overlay files, sync them back into the root:
   ```sh
   python3 addons/proxy/scripts/sync_overlay.py
   ```
5. If you need a buildable tree, materialize one:
   ```sh
   python3 addons/proxy/scripts/materialize_workspace.py --dest /tmp/janus-workspace
   ```

## Rollback

The current published proxy baseline is recorded in [`manifest.json`](./manifest.json) under `published_baseline`.

To restore addon-managed files back to that baseline:

```sh
python3 addons/proxy/scripts/rollback_to_published.py
```

That restores the overlay files from the baseline commit and re-syncs the repo-owned overlay targets into the root tree.

## Enforcement

- `python3 addons/proxy/scripts/sync_overlay.py --check` verifies repo-owned overlay files match the root tree.
- `python3 addons/proxy/scripts/check_compat.py` verifies repo-owned overlay sync and the pinned upstream blob hashes.
- `proxy-upstream-compat` runs those checks in CI.
- `proxy-release` materializes a temporary workspace from `upstream/codex` plus the overlay before building.
