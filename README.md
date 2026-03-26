# Janus Proxy Addon

This repository is not a full Codex fork anymore.

- The upstream Codex source lives in the pinned submodule at [`upstream/codex`](./upstream/codex).
- The root repository carries only addon-owned repo files: release workflows, the Pages site, the user-facing proxy docs, and addon scripts.
- Releases are built from a temporary workspace created from `upstream/codex` plus the overlay in [`addons/proxy/overlay`](./addons/proxy/overlay).

## Clone And Inspect

After cloning, initialize the upstream submodule:

```sh
git submodule update --init --recursive
```

The upstream tree is then visible under [`upstream/codex`](./upstream/codex). There is intentionally no second live copy of `codex-cli`, `codex-rs`, `sdk`, or the upstream docs in the repo root.

## Repository Layout

- [`upstream/codex`](./upstream/codex): pinned pristine upstream Codex checkout
- [`addons/proxy/overlay`](./addons/proxy/overlay): only the files that are injected into a materialized upstream workspace during build/release
- [`addons/proxy/scripts`](./addons/proxy/scripts): compatibility, rollback, and workspace-materialization scripts
- [`README-proxy.md`](./README-proxy.md): user-facing proxy usage guide
- [`site`](./site): static Pages site for the proxy package
- [`.github/workflows/proxy-release.yml`](./.github/workflows/proxy-release.yml): release/publish workflow

## How The Build Works

The repo root is not buildable as a Codex workspace on its own. The release flow does this instead:

1. checks out `upstream/codex`
2. validates the pinned upstream blob hashes for the workspace overlay
3. materializes a temporary workspace from the submodule with:
   ```sh
   python3 addons/proxy/scripts/materialize_workspace.py --dest /tmp/janus-workspace
   ```
4. builds and packages from that prepared workspace

## Updating Upstream

1. Move the submodule:
   ```sh
   git submodule update --remote upstream/codex
   ```
2. Review any overlay breakage:
   ```sh
   python3 addons/proxy/scripts/check_compat.py
   ```
3. If the new upstream state is accepted, refresh pinned blob hashes:
   ```sh
   python3 addons/proxy/scripts/update_manifest.py
   ```

## Docs

- [Proxy usage](./README-proxy.md)
- [Addon architecture](./addons/proxy/README.md)
- [Published npm package README source](./addons/proxy/overlay/codex-rs/responses-api-proxy/npm/README.md)
