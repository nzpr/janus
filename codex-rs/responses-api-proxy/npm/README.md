# @nzpr/codex-responses-api-proxy

<p align="center"><code>npm i -g @nzpr/codex-responses-api-proxy</code> to install <code>codex-responses-api-proxy</code></p>

`@nzpr/codex-responses-api-proxy` is a modified fork of OpenAI's Codex responses proxy. It is meant to be paired with the normal Codex CLI and can authenticate using your usual `~/.codex/auth.json` login, not only an API key.

It runs as a local proxy in front of Codex CLI, screens outbound requests for leaked secrets before forwarding them upstream, and can accept extra secrets from another local process over a Unix socket.

This package distributes the prebuilt [Codex Responses API proxy binary](https://github.com/nzpr/codex/tree/main/codex-rs/responses-api-proxy) for macOS and Linux.

## What This Is For

Use this package if you want:

- Codex CLI to keep using your normal ChatGPT or Codex CLI login from `auth.json`
- a local proxy layer between Codex CLI and the upstream responses endpoint
- secret leak detection before requests leave your machine
- an optional Unix socket where another local process can push extra secrets to redact

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

### Push Extra Secrets Over A Unix Socket

If you want another local process to supply additional secrets for redaction, start the proxy with `--secret-socket /tmp/codex-secrets.sock`:

```shell
codex-responses-api-proxy \
  --auth-json \
  --secret-socket /tmp/codex-secrets.sock \
  --http-shutdown \
  --server-info /tmp/server-info.json
```

Then connect to that Unix socket with either:

- a JSON array of strings
- newline-delimited strings

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

Each socket write replaces the previous socket-provided list for subsequent requests.

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
- `--secret-socket` is for cases where another local process already knows about secrets that should be redacted.
- The main use case is Codex CLI with normal `auth.json` auth plus secret screening.
