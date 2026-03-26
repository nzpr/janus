# Janus Proxy Binary

`codex-responses-api-proxy` is the binary behind Janus Proxy for Codex CLI.

It only forwards `POST` requests to `/v1/responses`, injecting an `Authorization: Bearer ...` header from either `stdin` or Codex auth storage. Before forwarding, it redacts only the secret values that were explicitly supplied over the optional Unix socket. Everything else is rejected with `403 Forbidden`.

It can also listen on an optional Unix socket for externally supplied secret values. Each socket write replaces the current socket-provided secret list, and only those values are redacted.

## Expected Usage

**IMPORTANT:** `codex-responses-api-proxy` is designed to be run by a privileged user with access to the bearer credential it will use so that an unprivileged user cannot inspect or tamper with the process. Though if `--http-shutdown` is specified, an unprivileged user _can_ make a `GET` request to `/shutdown` to shutdown the server, as an unprivileged user could not send `SIGTERM` to kill the process.

A privileged user (i.e., `root` or a user with `sudo`) who has access to `OPENAI_API_KEY` would run the following to start the server, as `codex-responses-api-proxy` reads the auth token from `stdin`:

```shell
printenv OPENAI_API_KEY | env -u OPENAI_API_KEY codex-responses-api-proxy --http-shutdown --server-info /tmp/server-info.json
```

If you want to reuse your existing Codex login in `CODEX_HOME/auth.json`, run:

```shell
codex-responses-api-proxy --auth-json --http-shutdown --server-info /tmp/server-info.json
```

If another local process needs to provide additional secret values to redact, add `--secret-socket` and send one of these payload shapes over that socket:

- JSON array of raw secret strings
- JSON object of `NAME: value` pairs
- newline-delimited raw secret strings
- newline-delimited `NAME=value` or `NAME: value` entries

For `NAME=value` / object input, the proxy only uses the values for redaction so env var names remain visible. Each write replaces the previous socket-provided list:

```shell
codex-responses-api-proxy \
  --auth-json \
  --secret-socket /tmp/codex-secrets.sock \
  --http-shutdown \
  --server-info /tmp/server-info.json

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

When `--auth-json` resolves a ChatGPT login token, the proxy automatically switches its default upstream to `https://chatgpt.com/backend-api/codex/responses` and adds the `ChatGPT-Account-ID` header when present.

A non-privileged user would then run Codex as follows, specifying the `model_provider` dynamically:

```shell
PROXY_PORT=$(jq .port /tmp/server-info.json)
PROXY_BASE_URL="http://127.0.0.1:${PROXY_PORT}"
codex exec -c "model_providers.openai-proxy={ name = 'OpenAI Proxy', base_url = '${PROXY_BASE_URL}/v1', wire_api='responses' }" \
    -c model_provider="openai-proxy" \
    'Your prompt here'
```

When the unprivileged user was finished, they could shutdown the server using `curl` (since `kill -SIGTERM` is not an option):

```shell
curl --fail --silent --show-error "${PROXY_BASE_URL}/shutdown"
```

## Behavior

- Reads a bearer token from `stdin` by default. All callers should pipe the token in (for example, `printenv OPENAI_API_KEY | codex-responses-api-proxy`).
- Alternatively, `--auth-json` loads auth from `CODEX_HOME/auth.json` (default `~/.codex/auth.json`) and supports ChatGPT login tokens from that file.
- Formats the header value as `Bearer <token>` and attempts to `mlock(2)` the memory holding that header so it is not swapped to disk.
- Listens on the provided port or an ephemeral port if `--port` is not specified.
- Accepts exactly `POST /v1/responses` (no query string). The request body is sanitized only by replacing explicit socket-provided secret values, then forwarded upstream with `Authorization: Bearer <token>` set. All original request headers (except any incoming `Authorization`) are forwarded upstream, with `Host` overridden to the upstream host. ChatGPT auth additionally forwards `ChatGPT-Account-ID` when available. For other requests, it responds with `403`.
- Optionally writes a single-line JSON file with server info, currently `{ "port": <u16>, "pid": <u32> }`.
- Optional `--http-shutdown` enables `GET /shutdown` to terminate the process with exit code `0`. This allows one user (e.g., `root`) to start the proxy and another unprivileged user on the host to shut it down.

## CLI

```
codex-responses-api-proxy [--port <PORT>] [--server-info <FILE>] [--http-shutdown] [--auth-json] [--codex-home <DIR>] [--upstream-url <URL>] [--secret-socket <PATH>]
```

- `--port <PORT>`: Port to bind on `127.0.0.1`. If omitted, an ephemeral port is chosen.
- `--server-info <FILE>`: If set, the proxy writes a single line of JSON with `{ "port": <PORT>, "pid": <PID> }` once listening.
- `--http-shutdown`: If set, enables `GET /shutdown` to exit the process with code `0`.
- `--auth-json`: Load auth from `CODEX_HOME/auth.json` instead of `stdin`. This supports ChatGPT login tokens from `auth.json`.
- `--codex-home <DIR>`: Override the Codex home directory used by `--auth-json`. Defaults to `CODEX_HOME` or `~/.codex`.
- `--upstream-url <URL>`: Absolute URL to forward requests to. Defaults to `https://api.openai.com/v1/responses` for API-key auth and `https://chatgpt.com/backend-api/codex/responses` for ChatGPT auth.
- `--secret-socket <PATH>`: Bind a Unix socket that accepts a JSON array of secret strings, a JSON object of `NAME: value` pairs, or newline-delimited UTF-8 entries. For `NAME=value` / object input, only the values are redacted. The latest received list fully defines which values are redacted on subsequent requests.
- Authentication is fixed to `Authorization: Bearer <token>` to match the Codex CLI expectations.

For Azure, for example (ensure your deployment accepts `Authorization: Bearer <token>`):

```shell
printenv AZURE_OPENAI_API_KEY | env -u AZURE_OPENAI_API_KEY codex-responses-api-proxy \
  --http-shutdown \
  --server-info /tmp/server-info.json \
  --upstream-url "https://YOUR_PROJECT_NAME.openai.azure.com/openai/deployments/YOUR_DEPLOYMENT/responses?api-version=2025-04-01-preview"
```

## Notes

- Only `POST /v1/responses` is permitted. No query strings are allowed.
- All request headers are forwarded to the upstream call (aside from overriding `Authorization` and `Host`, plus setting `ChatGPT-Account-ID` when `--auth-json` resolves ChatGPT auth). Response status and content-type are mirrored from upstream.
- No automatic file, environment, git-remote, or regex-based secret discovery is performed. Only the values explicitly supplied over `--secret-socket` are redacted.

## Hardening Details

Care is taken to restrict access/copying to the bearer credential retained in memory:

- We leverage [`codex_process_hardening`](../process-hardening/README.md) so `codex-responses-api-proxy` is run with standard process-hardening techniques.
- At startup, we allocate a `1024` byte buffer on the stack and copy `"Bearer "` into the start of the buffer.
- We then read from `stdin`, copying the contents into the buffer after `"Bearer "`.
- After verifying the resulting header is a valid HTTP header value (and does not exceed the buffer), we create a `String` from that buffer (so the data is now on the heap).
- We zero out the stack-allocated buffer using https://crates.io/crates/zeroize so it is not optimized away by the compiler.
- We invoke `.leak()` on the `String` so we can treat its contents as a `&'static str`, as it will live for the rest of the process.
- On UNIX, we `mlock(2)` the memory backing the `&'static str`.
- When using the `&'static str` when building an HTTP request, we use `HeaderValue::from_static()` to avoid copying the `&str`.
- We also invoke `.set_sensitive(true)` on the `HeaderValue`, which in theory indicates to other parts of the HTTP stack that the header should be treated with "special care" to avoid leakage:

https://github.com/hyperium/http/blob/439d1c50d71e3be3204b6c4a1bf2255ed78e1f93/src/header/value.rs#L346-L376
