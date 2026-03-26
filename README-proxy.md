# Janus Proxy For Codex CLI

Janus Proxy is a local `/v1/responses` proxy for Codex CLI.

Package name:

```shell
@nzpr/janus
```

Command name:

```shell
janus
```

What it does:

- accepts only `POST /v1/responses`
- injects the upstream bearer credential
- redacts only the secret values explicitly supplied over an optional Unix socket
- can use either standard Codex auth storage or a token from `stdin`

The easiest install path is the npm package:

```shell
npm i -g @nzpr/janus
```

The binary is built from the pinned upstream Codex submodule plus the overlay under [`addons/proxy/overlay`](./addons/proxy/overlay), not from a second checked-in `codex-rs` tree in the repository root.

## Typical Setup

### 1. Start The Proxy

Using standard Codex auth storage:

```shell
janus --auth-json --http-shutdown --server-info /tmp/server-info.json
```

Using a token from `stdin`:

```shell
printenv OPENAI_API_KEY | env -u OPENAI_API_KEY \
  janus --http-shutdown --server-info /tmp/server-info.json
```

If you need a non-default Codex home:

```shell
janus \
  --auth-json \
  --codex-home /path/to/codex-home \
  --http-shutdown \
  --server-info /tmp/server-info.json
```

### 2. Optionally Push Extra Secrets Over A Unix Socket

If another local process already knows about secrets that should be redacted, start the proxy with a socket path. Only the values you send over that socket will be filtered:

```shell
janus \
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

Each write replaces the previous socket-provided list. If you do not send any secrets, the proxy does not redact anything from request bodies.

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

## Auth Behavior

When `--auth-json` resolves ChatGPT auth from `auth.json`, the proxy automatically:

- switches its default upstream to `https://chatgpt.com/backend-api/codex/responses`
- forwards `ChatGPT-Account-ID` when present

This means users who already sign into Codex with ChatGPT typically do not need a separate API key to start the proxy.

## Where To Look Next

- npm package guide source: [`addons/proxy/overlay/codex-rs/responses-api-proxy/npm/README.md`](./addons/proxy/overlay/codex-rs/responses-api-proxy/npm/README.md)
- crate-level details and CLI reference source: [`addons/proxy/overlay/codex-rs/responses-api-proxy/README.md`](./addons/proxy/overlay/codex-rs/responses-api-proxy/README.md)
