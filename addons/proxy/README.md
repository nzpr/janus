# Proxy Addon

This directory is the first-party layer that sits on top of the upstream Codex submodule in [`upstream/codex`](../../upstream/codex).

## Purpose

Keep our proxy-specific work clearly separated from upstream while still letting us update upstream Codex with a normal submodule bump.

## Current Model

- `upstream/codex` is a pinned git submodule that tracks `https://github.com/openai/codex.git`.
- [`overlay/`](./overlay/) is the source-of-truth for the addon-owned files we keep on top of the root workspace.
- The repository root still contains the live files that releases and docs currently use, but they must match the overlay exactly.
- [`manifest.json`](./manifest.json) records:
  - the pinned upstream commit
  - the currently published rollback baseline
  - the managed file mapping from overlay paths to live root paths
  - the upstream blob revisions for files we override from upstream

## Why This Is Incremental

The repo already contains a working fork layout and release machinery. Moving every live path out of the root in one step would create unnecessary churn. This addon structure establishes the upstream boundary first, adds compatibility detection, and keeps the current release paths working.

## Update Flow

1. Update the submodule:
   ```sh
   git submodule update --remote upstream/codex
   ```
2. If you changed addon-owned files under `overlay/`, sync them into the live root tree:
   ```sh
   python3 addons/proxy/scripts/sync_overlay.py
   ```
3. Run the compatibility checker:
   ```sh
   python3 addons/proxy/scripts/check_compat.py
   ```
4. Review any upstream file changes that touched our overlay surface.
5. If the new upstream state is compatible, refresh the pinned blob hashes:
   ```sh
   python3 addons/proxy/scripts/update_manifest.py
   ```
6. Re-run the checker and the proxy validation suite.

## Rollback

The current published proxy baseline is recorded in [`manifest.json`](./manifest.json) under `published_baseline`.

To restore the addon-managed proxy surface back to that published version:

```sh
python3 addons/proxy/scripts/rollback_to_published.py
```

That restores the overlay files from the published baseline commit and then syncs them back into the live root tree.

## Overlay Surface

The key upstream-backed overlay today is the Responses API proxy implementation under:

- `codex-rs/responses-api-proxy/`

The main first-party addon files include:

- `codex-rs/responses-api-proxy/src/screening.rs`
- `codex-rs/responses-api-proxy/src/secret_socket.rs`
- `.github/workflows/proxy-release.yml`
- `.github/workflows/proxy-pages.yml`
- `README-proxy.md`
- `site/`

Those files do not have direct upstream counterparts and are owned entirely by this addon layer.

## Enforcement

- `python3 addons/proxy/scripts/check_compat.py` fails if:
  - the root files drift from `overlay/`
  - a pinned upstream-backed file changed in the submodule since the last reviewed manifest update
- `proxy-upstream-compat` runs that check in CI
- `proxy-release` and `proxy-pages` both verify overlay sync before publishing
