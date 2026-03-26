# Janus Proxy For Codex CLI

<p align="center"><code>npm i -g @nzpr/janus</code> to install <code>codex-responses-api-proxy</code></p>

`@nzpr/janus` is the npm package for Janus Proxy.

Janus Proxy is a local `/v1/responses` proxy for Codex CLI. It accepts only `POST /v1/responses`, injects the upstream bearer credential, and redacts only the secret values that another local process explicitly sends over a Unix socket.

This package distributes the prebuilt proxy binary for macOS and Linux.

## What This Is For

Use this package if you want:

- a local proxy layer between Codex CLI and the upstream responses endpoint
- explicit secret redaction before requests leave your machine
- an optional Unix socket where another local process can push extra secrets to redact
- compatibility with normal Codex CLI usage

This package does not replace Codex CLI. You install Codex separately and point it at this proxy.

## Quickstart

Install the package globally:

```shell
npm i -g @nzpr/janus
```

Confirm the binary is available:

```shell
codex-responses-api-proxy --help
```

### Start The Proxy

Using standard Codex auth storage:

```shell
codex-responses-api-proxy --auth-json --http-shutdown --server-info /tmp/server-info.json
```

Using a token from `stdin`:

```shell
printenv OPENAI_API_KEY | env -u OPENAI_API_KEY \
  codex-responses-api-proxy --http-shutdown --server-info /tmp/server-info.json
```

If the auth in `auth.json` is a ChatGPT login, the proxy automatically uses `https://chatgpt.com/backend-api/codex/responses` and forwards `ChatGPT-Account-ID` when present.

### Push Extra Secrets Over A Unix Socket

If you want another local process to supply additional secrets for redaction, start the proxy with `--secret-socket /tmp/codex-secrets.sock`. Only the values sent over that socket are filtered:

```shell
codex-responses-api-proxy \
  --auth-json \
  --secret-socket /tmp/codex-secrets.sock \
  --http-shutdown \
  --server-info /tmp/server-info.json
```

Then connect to that Unix socket with either:

- a JSON array of strings
- a JSON object of `NAME: value` pairs
- newline-delimited strings
- newline-delimited `NAME=value` or `NAME: value` entries

Example:

```shell
python3 - <<'PY'
import json
import socket

payload = json.dumps(["internal-token-1", "db-password-2"]).encode()
sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
sock.connect("/tmp/codex-secrets.sock")
sock.sendall(payload)
sock.close()
PY
```

For `NAME=value` / object input, the proxy uses only the values for redaction so env var names stay visible. Each socket write replaces the previous socket-provided list for subsequent requests. If you never send any secrets, nothing is redacted.

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

- [`README-proxy.md`](https://github.com/nzpr/janus/blob/main/README-proxy.md)
- [`addons/proxy/overlay/codex-rs/responses-api-proxy/README.md`](https://github.com/nzpr/janus/blob/main/addons/proxy/overlay/codex-rs/responses-api-proxy/README.md)

## Notes

- macOS and Linux vendor binaries are included in the npm package.
- `--server-info` is the easiest way to discover the local port that was selected.
- `--secret-socket` is the only source of redacted secret values.
- Janus Proxy is the product name; the installable package is `@nzpr/janus`.
