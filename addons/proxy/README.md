# Proxy Addon

This directory is the first-party layer that sits on top of the upstream Codex submodule in [`upstream/codex`](../../upstream/codex).

## Purpose

Keep our proxy-specific work clearly separated from upstream while still letting us update upstream Codex with a normal submodule bump.

## Current Model

- `upstream/codex` is a pinned git submodule that tracks `https://github.com/openai/codex.git`.
- The repository root still contains the live overlay files that our releases and docs currently use.
- [`manifest.json`](./manifest.json) records which of those files are:
  - overlays on top of upstream files
  - purely first-party addon files
- [`scripts/check_compat.py`](./scripts/check_compat.py) verifies that the upstream files we overlay still match the blob revisions we last reviewed.

## Why This Is Incremental

The repo already contains a working fork layout and release machinery. Moving every live path out of the root in one step would create unnecessary churn. This addon structure establishes the upstream boundary first, adds compatibility detection, and keeps the current release paths working.

## Update Flow

1. Update the submodule:
   ```sh
   git submodule update --remote upstream/codex
   ```
2. Run the compatibility checker:
   ```sh
   python3 addons/proxy/scripts/check_compat.py
   ```
3. Review any upstream file changes that touched our overlay surface.
4. If the new upstream state is compatible, refresh the pinned blob hashes:
   ```sh
   python3 addons/proxy/scripts/update_manifest.py
   ```
5. Re-run the checker and the proxy validation suite.

## Overlay Surface

The key upstream-backed overlay today is the Responses API proxy implementation under:

- `codex-rs/responses-api-proxy/`

First-party addon files include:

- `codex-rs/responses-api-proxy/src/screening.rs`
- `codex-rs/responses-api-proxy/src/secret_socket.rs`
- `.github/workflows/proxy-release.yml`
- `.github/workflows/proxy-pages.yml`
- `README-proxy.md`
- `site/`

Those files do not have direct upstream counterparts and are owned entirely by this addon layer.
