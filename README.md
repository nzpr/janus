# Janus

Janus is proxy for agents that filters secrets. 
Atm only for Codex CLI.

# The problem

Even if you jail your agent inside a container, if you want high autonomy you prob want it to run tools that require secrets. Though you might attempt to inject secrets 
at the container or network boundary, this is not always possible. For HTTP its quite easy. But for other protocols and tools it may be cumbersome.
So in the end you just place secrets inside the jail next to the agent. 

You might think big companies do not need my secrets. Which may be the case. And since avalanche of secret leaks happened already - 
agents know how to handle this. But still tricy attacks are possible that can lead to losses, so its better to make sure that LLM never sees your secrets.

So this is what Janus aims to solve - filter secrets out of your conversation with agent. Anything response that leaves your computer is scanned and potential secrets leaked are redacted into dummy word.

It is based on the upstream OpenAI responses proxy, but packaged and documented for Codex CLI usage. The current Janus build is intentionally narrow:

- it accepts only `POST /v1/responses`
- it injects the upstream bearer credential itself
- it redacts only secret values explicitly supplied over its Unix socket
- it is meant to sit locally between Codex and the upstream responses endpoint

The installable package is [`@nzpr/janus`](https://www.npmjs.com/package/@nzpr/janus). The installed command and standalone binary are both named `janus`.

## Quickstart

Install:

```sh
npm install -g @nzpr/janus
```

Start the proxy:

```sh
janus --auth-json --http-shutdown --server-info /tmp/server-info.json
```

Point Codex at it:

```sh
PROXY_PORT=$(jq .port /tmp/server-info.json)
PROXY_BASE_URL="http://127.0.0.1:${PROXY_PORT}"

codex exec \
  -c "model_providers.janus={ name='Janus', base_url='${PROXY_BASE_URL}/v1', wire_api='responses' }" \
  -c "model_provider='janus'" \
  "Your prompt here"
```

## What Janus Does

- Runs as a loopback-only local proxy on `127.0.0.1`
- Forwards only the Responses API route Codex needs
- Can read credentials from Codex auth storage with `--auth-json` or from `stdin`
- Can accept a Unix socket feed of secret values that should be redacted before forwarding

If no secrets are sent to `--secret-socket`, Janus does not redact anything from the request body.

## Documentation

- [User guide](./README-proxy.md)
- [npm package README source](./addons/proxy/overlay/codex-rs/responses-api-proxy/npm/README.md)
- [binary/crate README source](./addons/proxy/overlay/codex-rs/responses-api-proxy/README.md)
- [project site](./site)

## Repository Layout

- [`upstream/codex`](./upstream/codex): pinned upstream Codex submodule
- [`addons/proxy/overlay`](./addons/proxy/overlay): Janus overlay applied on top of upstream
- [`addons/proxy/scripts`](./addons/proxy/scripts): compatibility and materialization tooling
- [`.github/workflows/proxy-release.yml`](./.github/workflows/proxy-release.yml): release workflow

Initialize the upstream submodule after cloning:

```sh
git submodule update --init --recursive
```
