# @nzpr/codex-responses-api-proxy

<p align="center"><code>npm i -g @nzpr/codex-responses-api-proxy</code> to install <code>codex-responses-api-proxy</code></p>

`@nzpr/codex-responses-api-proxy` is a modified fork of OpenAI's Codex responses proxy. It is meant to be paired with the normal Codex CLI and can authenticate using your usual `~/.codex/auth.json` login, not only an API key.

It runs as a local proxy in front of Codex CLI and screens outbound requests for leaked secrets before forwarding them upstream.

This package distributes the prebuilt [Codex Responses API proxy binary](https://github.com/nzpr/codex/tree/main/codex-rs/responses-api-proxy) for macOS and Linux.

## What This Is For

Use this package if you want:

- Codex CLI to keep using your normal ChatGPT or Codex CLI login from `auth.json`
- a local proxy layer between Codex CLI and the upstream responses endpoint
- secret leak detection before requests leave your machine

This package does not replace Codex CLI. You install Codex separately and point it at this proxy.

## Quickstart

Install the package globally:

```shell
npm i -g @nzpr/codex-responses-api-proxy
```

Confirm the binary is available:

```shell
codex-responses-api-proxy --help
```

### Use Your Existing Codex Login

If you already use Codex CLI with `auth.json`, start the proxy like this:

```shell
codex-responses-api-proxy --auth-json --http-shutdown --server-info /tmp/server-info.json
```

This reads auth from `CODEX_HOME/auth.json` (default `~/.codex/auth.json`).

If the auth in `auth.json` is a ChatGPT login, the proxy automatically:

- uses `https://chatgpt.com/backend-api/codex/responses` as the upstream
- forwards `ChatGPT-Account-ID` when present

### Use An API Key

If you want to start the proxy with an API key instead:

```shell
printenv OPENAI_API_KEY | env -u OPENAI_API_KEY \
  codex-responses-api-proxy --http-shutdown --server-info /tmp/server-info.json
```

### Point Codex At The Proxy

Read the port from the startup file:

```shell
PROXY_PORT=$(jq .port /tmp/server-info.json)
PROXY_BASE_URL="http://127.0.0.1:${PROXY_PORT}"
```

Run Codex through the proxy:

```shell
codex exec \
  -c "model_providers.openai_proxy={ name='OpenAI Proxy', base_url='${PROXY_BASE_URL}/v1', wire_api='responses' }" \
  -c "model_provider='openai_proxy'" \
  "Your prompt here"
```

You can use the same `-c` settings with interactive `codex` as well.

When finished, stop the proxy:

```shell
curl --fail --silent --show-error "${PROXY_BASE_URL}/shutdown"
```

## More Docs

For the full CLI reference and behavior details, see:

- [`README-proxy.md`](https://github.com/nzpr/codex/blob/main/README-proxy.md)
- [`codex-rs/responses-api-proxy/README.md`](https://github.com/nzpr/codex/blob/main/codex-rs/responses-api-proxy/README.md)

## Notes

- macOS and Linux vendor binaries are included in the npm package.
- `--auth-json` is the easiest option if you already use Codex CLI with ChatGPT sign-in.
- `--server-info` is the easiest way to discover the local port that was selected.
- The main use case is Codex CLI with normal `auth.json` auth plus secret screening.
