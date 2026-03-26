# Proxy Addon

This directory is the first-party layer that sits on top of the upstream Codex submodule in [`upstream/codex`](../../upstream/codex).

## Final Model

- `upstream/codex` is the only copy of the upstream Codex source in this repository.
- The repository root holds addon-owned repo files only: proxy workflows, the Pages site, the user-facing docs, and the addon scripts.
- [`overlay/`](./overlay/) contains the files that are applied on top of a materialized copy of `upstream/codex` for build and release work.
- [`manifest.json`](./manifest.json) records:
  - the pinned upstream commit
  - the currently published rollback baseline
  - which overlay files patch the materialized upstream workspace
  - the upstream blob revisions that were last reviewed

## Materialized Workspace

To create a buildable temporary workspace from upstream plus the overlay:

```sh
python3 addons/proxy/scripts/materialize_workspace.py --dest /tmp/janus-workspace
```

That exports `upstream/codex` into the destination and then copies every overlay file into its target path inside that temporary tree.

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
4. If you need a buildable tree, materialize one:
   ```sh
   python3 addons/proxy/scripts/materialize_workspace.py --dest /tmp/janus-workspace
   ```

## Rollback

The current published proxy baseline is recorded in [`manifest.json`](./manifest.json) under `published_baseline`.

To restore addon-managed files back to that baseline:

```sh
python3 addons/proxy/scripts/rollback_to_published.py
```

That restores the overlay files from the baseline commit.

## Enforcement

- `python3 addons/proxy/scripts/check_compat.py` verifies the pinned upstream blob hashes for the workspace overlay.
- `proxy-upstream-compat` runs those checks in CI.
- `proxy-release` materializes a temporary workspace from `upstream/codex` plus the overlay before building.
