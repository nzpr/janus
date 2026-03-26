# Janus Proxy

Janus Proxy is a local `/v1/responses` proxy for Codex CLI.

It is meant for users who want one narrow control point between Codex and the upstream responses endpoint. The proxy:

- accepts only `POST /v1/responses`
- injects the bearer credential used for the upstream call
- redacts only the secret values explicitly supplied over its Unix socket
- can be paired with normal Codex CLI usage

The installable package is [`@nzpr/janus`](https://www.npmjs.com/package/@nzpr/janus). The shipped command and standalone binary are named `janus`.

## Start Here

- [Proxy usage guide](./README-proxy.md)
- [npm package README source](./addons/proxy/overlay/codex-rs/responses-api-proxy/npm/README.md)
- [Binary/crate README source](./addons/proxy/overlay/codex-rs/responses-api-proxy/README.md)

## Repository Structure

- [`upstream/codex`](./upstream/codex): pinned upstream Codex source
- [`addons/proxy/overlay`](./addons/proxy/overlay): files applied on top of the upstream workspace for build/release
- [`addons/proxy/scripts`](./addons/proxy/scripts): compatibility, rollback, and workspace-materialization scripts
- [`site`](./site): static project site
- [`.github/workflows/proxy-release.yml`](./.github/workflows/proxy-release.yml): release workflow

After cloning, initialize the upstream submodule:

```sh
git submodule update --init --recursive
```

## Build Model

This repo does not keep a second live copy of the upstream Codex tree at the root.

Release builds work by:

1. checking out [`upstream/codex`](./upstream/codex)
2. validating the reviewed upstream blob hashes
3. materializing a temporary workspace with:
   ```sh
   python3 addons/proxy/scripts/materialize_workspace.py --dest /tmp/janus-workspace
   ```
4. building and packaging from that prepared workspace
