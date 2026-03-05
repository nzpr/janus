# Janus

Janus is a host-side secret broker daemon for sandboxed LLM agents.

It is **not MCP-based**. Run it on the host, keep secrets on the host, and hand sandboxed clients only short-lived capability env values.

Published repository: `https://github.com/nzpr/janus`

## Quick Start

1. Build/run the daemon:

```bash
make start
```

This starts `janusd` with defaults:
- proxy bind: `127.0.0.1:9080`
- control API socket: `/tmp/janusd-control.sock`

2. Export host secrets (host only):

```bash
export JANUS_GIT_HTTP_PASSWORD=your-token
# optional
# export JANUS_GIT_HTTP_USERNAME=x-access-token
# export JANUS_GIT_HTTP_HOSTS=github.com,gitlab.com
```

3. Create a session from host control API:

```bash
curl --unix-socket /tmp/janusd-control.sock \
  -s -X POST http://localhost/v1/sessions
```

4. Apply returned `env` map to the sandboxed runtime.

## How It Works

- `janusd` keeps upstream credentials in host env only.
- Sandbox receives only session-scoped capability values (proxy URL + rewrites).
- HTTP/Git traffic is brokered through Janus proxy.
- For host-native tooling (for example `psql`, `kubectl`, `terraform`, `ssh`), use:
  - `POST /v1/exec` on the control socket.

## Protocol Coverage Model

- First-class data plane: HTTP(S), Git-over-HTTP.
- Host-exec adapters: Postgres/deployment tooling via allowlisted command execution.
- Future protocols: add adapters without exposing raw credentials to sandbox.

## Safety Model

- Secrets are never returned by control API.
- Sessions are short-lived and host-scoped.
- Outbound hosts are allowlisted per session.
- Daemon boundary is the enforcement and audit point.

## Environment Variables

Core:
- `JANUS_PROXY_BIND` (default `127.0.0.1:9080`)
- `JANUS_CONTROL_SOCKET` (default `/tmp/janusd-control.sock`)
- `JANUS_DEFAULT_TTL_SECONDS` (default `3600`)
- `JANUS_ALLOWED_HOSTS` (default `github.com,api.github.com,gitlab.com`)

Git auth:
- `JANUS_GIT_HTTP_PASSWORD` or `JANUS_GIT_HTTP_TOKEN`
- `JANUS_GIT_HTTP_USERNAME` (default `x-access-token`)
- `JANUS_GIT_HTTP_HOSTS` (default `github.com`)

Host exec:
- `JANUS_EXEC_ALLOWLIST` (default `git,psql,kubectl,helm,terraform,ssh`)
- optional Postgres defaults: `JANUS_POSTGRES_HOST`, `JANUS_POSTGRES_PORT`, `JANUS_POSTGRES_USER`, `JANUS_POSTGRES_DATABASE`, `JANUS_POSTGRES_PASSWORD`

UI:
- `JANUS_NO_BANNER=1` disables startup banner.

## License And Warranty

Licensed under MIT. See [LICENSE](./LICENSE).

This software is provided **"AS IS"**, without warranty of any kind, express or implied.
