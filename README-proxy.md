# Codex Responses API Proxy

This repository includes `codex-responses-api-proxy`, a modified fork of OpenAI's Codex responses proxy.

It is intended to be paired with the normal Codex CLI and adds two important things for that flow:

- support for the usual Codex CLI `auth.json` / ChatGPT login flow, not only API-key auth
- secret screening before requests are forwarded upstream

At runtime it behaves as a local HTTP proxy for Codex that:

- accepts only `POST /v1/responses`
- injects `Authorization: Bearer ...`
- can read auth from `stdin` or `CODEX_HOME/auth.json`
- applies secret screening before forwarding requests upstream
- can merge in extra secret values received over an optional Unix socket

The easiest install path is the npm package:

```shell
npm i -g @nzpr/codex-responses-api-proxy
```

## Typical Setup

### 1. Start The Proxy With Existing Codex Auth

If you already use Codex with ChatGPT sign-in and have `~/.codex/auth.json`:

```shell
codex-responses-api-proxy --auth-json --http-shutdown --server-info /tmp/server-info.json
```

If you need to point at a different Codex home:

```shell
codex-responses-api-proxy \
  --auth-json \
  --codex-home /path/to/codex-home \
  --http-shutdown \
  --server-info /tmp/server-info.json
```

### 2. Or Start It With An API Key

```shell
printenv OPENAI_API_KEY | env -u OPENAI_API_KEY \
  codex-responses-api-proxy --http-shutdown --server-info /tmp/server-info.json
```

### 2a. Optionally Push Extra Secrets Over A Unix Socket

If another local process already knows about secrets that should be redacted, start the proxy with a socket path:

```shell
codex-responses-api-proxy \
  --auth-json \
  --secret-socket /tmp/codex-secrets.sock \
  --http-shutdown \
  --server-info /tmp/server-info.json
```

Then push the complete replacement list as either:

- a JSON array of raw secret strings
- a JSON object of `NAME: value` pairs
- newline-delimited raw secret strings
- newline-delimited `NAME=value` or `NAME: value` entries

For `NAME=value` / object input, the proxy uses only the values for redaction so env var names stay visible. Example with Python:

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

Each write replaces the previous socket-provided list.

### 3. Read The Assigned Port

```shell
PROXY_PORT=$(jq .port /tmp/server-info.json)
PROXY_BASE_URL="http://127.0.0.1:${PROXY_PORT}"
```

### 4. Run Codex Through The Proxy

One-shot command:

```shell
codex exec \
  -c "model_providers.openai_proxy={ name='OpenAI Proxy', base_url='${PROXY_BASE_URL}/v1', wire_api='responses' }" \
  -c "model_provider='openai_proxy'" \
  "Your prompt here"
```

Interactive session:

```shell
codex \
  -c "model_providers.openai_proxy={ name='OpenAI Proxy', base_url='${PROXY_BASE_URL}/v1', wire_api='responses' }" \
  -c "model_provider='openai_proxy'"
```

### 5. Stop The Proxy

```shell
curl --fail --silent --show-error "${PROXY_BASE_URL}/shutdown"
```

## What `--auth-json` Does

When `--auth-json` resolves ChatGPT auth from `auth.json`, the proxy automatically:

- switches its default upstream to `https://chatgpt.com/backend-api/codex/responses`
- forwards `ChatGPT-Account-ID` when present

This means users who already sign into Codex with ChatGPT typically do not need an API key to start the proxy.

## Where To Look Next

- npm package guide: [`codex-rs/responses-api-proxy/npm/README.md`](./codex-rs/responses-api-proxy/npm/README.md)
- crate-level details and CLI reference: [`codex-rs/responses-api-proxy/README.md`](./codex-rs/responses-api-proxy/README.md)
