# Janus

Janus is a host-side secret broker daemon for sandboxed LLM agents.

Janus runtime is **not MCP-coupled**. Run `janusd` on the host, keep secrets on the host, and give sandboxed runtimes only short-lived capability sessions.

Published repository: `https://github.com/nzpr/janus`

Go reimplementation: see [go/README.md](./go/README.md).

## Quick Start

1. Start server (no args):

```bash
make start
```

Defaults:
- proxy bind: `127.0.0.1:9080`
- control API socket: `/tmp/janusd-control.sock`

2. Set host secrets (host only):

```bash
export JANUS_GIT_HTTP_PASSWORD=your-token
# optional postgres defaults
# export JANUS_POSTGRES_HOST=localhost
# export JANUS_POSTGRES_USER=app
# export JANUS_POSTGRES_PASSWORD=...
```

3. Create a session from host control API:

```bash
curl --unix-socket /tmp/janusd-control.sock \
  -s -X POST http://localhost/v1/sessions
```

4. Apply returned `env` map to the sandbox runtime.

For full CLI docs:

```bash
janusd --help
```

## Optional MCP Companion (Read-Only)

If you want LLMs to discover Janus capabilities through MCP, run `janus-mcp` on the host.

- `janus-mcp` is metadata-only.
- It does not create sessions.
- It does not return secrets or tokens.
- It does not expose control socket path.

Build/run:

```bash
cargo run --bin janus-mcp -- --help
```

Concrete MCP config (Claude/Codex style):

```json
{
  "mcpServers": {
    "janus": {
      "command": "janus-mcp",
      "args": []
    }
  }
}
```

If running from source without install:

```json
{
  "mcpServers": {
    "janus": {
      "command": "cargo",
      "args": ["run", "--quiet", "--bin", "janus-mcp", "--"],
      "cwd": "/workspace"
    }
  }
}
```

## Control API

All endpoints are served on the Unix socket (`/tmp/janusd-control.sock` by default).

Create session with explicit capabilities:

```bash
curl --unix-socket /tmp/janusd-control.sock \
  -s -X POST http://localhost/v1/sessions \
  -H 'content-type: application/json' \
  -d '{
    "capabilities": ["http_proxy", "git_http", "postgres_query"],
    "allowed_hosts": ["github.com", "api.github.com"]
  }'
```

Postgres query adapter:

```bash
curl --unix-socket /tmp/janusd-control.sock \
  -s -X POST http://localhost/v1/postgres/query \
  -H 'content-type: application/json' \
  -d '{
    "session_id": "<session-id>",
    "sql": "select now();"
  }'
```

Deployment adapters:

```bash
# kubectl
curl --unix-socket /tmp/janusd-control.sock \
  -s -X POST http://localhost/v1/deploy/kubectl \
  -H 'content-type: application/json' \
  -d '{"session_id":"<session-id>","args":["get","pods","-A"]}'

# helm
curl --unix-socket /tmp/janusd-control.sock \
  -s -X POST http://localhost/v1/deploy/helm \
  -H 'content-type: application/json' \
  -d '{"session_id":"<session-id>","args":["list","-A"]}'

# terraform
curl --unix-socket /tmp/janusd-control.sock \
  -s -X POST http://localhost/v1/deploy/terraform \
  -H 'content-type: application/json' \
  -d '{"session_id":"<session-id>","args":["plan"],"cwd":"/infra"}'
```

## Capability Model

Known capabilities:
- `http_proxy`
- `git_http`
- `postgres_query`
- `deploy_kubectl`
- `deploy_helm`
- `deploy_terraform`

Default session capabilities:
- `http_proxy`
- `git_http`

## Safety Model

- Upstream credentials stay on host and are never returned by API.
- Session env map does not include the control socket path.
- No generic host shell endpoint (`/v1/exec` removed).
- Optional MCP companion is read-only metadata only.
- Every proxy/adapter request is capability-checked.
- Outbound hosts are allowlisted per session.
- Sensitive values are redacted from adapter stdout/stderr.
- Control API socket is created with mode `0600`.

Important deployment assumption:
- sandboxed agents must not have filesystem access to the host control socket path.

## Environment Variables

Core:
- `JANUS_PROXY_BIND` (default `127.0.0.1:9080`)
- `JANUS_CONTROL_SOCKET` (default `/tmp/janusd-control.sock`)
- `JANUS_DEFAULT_TTL_SECONDS` (default `3600`)
- `JANUS_DEFAULT_CAPABILITIES` (default `http_proxy,git_http`)
- `JANUS_ALLOWED_HOSTS` (default `github.com,api.github.com,gitlab.com`)

Git auth:
- `JANUS_GIT_HTTP_PASSWORD` or `JANUS_GIT_HTTP_TOKEN`
- `JANUS_GIT_HTTP_USERNAME` (default `x-access-token`)
- `JANUS_GIT_HTTP_HOSTS` (default `github.com`)

Postgres defaults (optional):
- `JANUS_POSTGRES_HOST`
- `JANUS_POSTGRES_PORT`
- `JANUS_POSTGRES_USER`
- `JANUS_POSTGRES_DATABASE`
- `JANUS_POSTGRES_PASSWORD`

Kubernetes tooling (optional):
- `JANUS_KUBECONFIG`

UI:
- `JANUS_NO_BANNER=1` disables startup banner.

## License And Warranty

Licensed under MIT. See [LICENSE](./LICENSE).

This software is provided **"AS IS"**, without warranty of any kind, express or implied.
