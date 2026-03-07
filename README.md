# Janus

Janus is a host-side secret broker daemon for sandboxed LLM agents.

Janus runtime is **not MCP-coupled**. Run `janusd` on the host, keep secrets on the host, and give sandboxed runtimes only short-lived capability sessions.

Published repository: `https://github.com/nzpr/janus`

## What Janus Is

Janus is a host-side broker for sandboxed LLM agents.

Core responsibilities:
- keep credentials on host only,
- issue short-lived capability sessions,
- enforce outbound policy through controlled proxy/adapters,
- provide typed host actions (for example deployment tooling) without exposing raw secrets to sandboxed agent code.

Policy:
- protocol access is provided via data-plane tunneling;
- control-plane adapters are reserved for operations not available on the data plane.

## Quick Start

User workflow:
1. start `janusd` on host (or Docker),
2. expose read-only discovery (`/health`, `/v1/config`) to sandbox if needed,
3. start `janus-mcp` and connect it to your LLM client,
4. let the LLM use MCP discovery/tools.  
You should not need to make manual proxy/control API calls in normal usage.

## Start Janusd (Host)

1. Set required host secret(s):

```bash
export JANUS_GIT_HTTP_PASSWORD=your-token
```

2. Start server:

```bash
make start
```

Defaults:
- proxy bind: `127.0.0.1:9080`
- control API socket: `/tmp/janusd-control.sock`
- public discovery API: disabled by default (`JANUS_DISCOVERY_BIND` unset)

3. Health check:

```bash
make health
```

For full CLI docs:

```bash
janusd --help
```

## Start Janusd (Docker)

```bash
cp .env.docker.example .env
make deploy
```

Then:

```bash
make health
make logs
```

If sandboxed MCP must use network discovery, expose discovery port too:

```bash
DISCOVERY_PORT=9181 make deploy
```

Stop:

```bash
make stop
```

`Makefile` deployment variables:
- `IMAGE` (default `janusd:latest`)
- `CONTAINER` (default `janusd`)
- `PROXY_PORT` (default `9080`)
- `DISCOVERY_PORT` (optional; when set also enables `JANUS_DISCOVERY_BIND=0.0.0.0:9181`)
- `SOCKET_DIR` (default `/tmp/janus`)
- `JANUS_ENV_FILE` (default `.env`)

## Start MCP Companion

`janus-mcp` is read-only metadata for LLM planning/discovery.
`janusd` lifecycle is always external. `janus-mcp` never starts `janusd`.

### Mode A: same host trust boundary (unix socket)

Run `janus-mcp` with direct socket access:

```bash
cargo run --bin janus-mcp -- --control-socket /tmp/janusd-control.sock
```

### Mode B: jailed LLM (recommended when MCP process is sandboxed)

1. On host, enable read-only discovery API:

```bash
export JANUS_DISCOVERY_BIND=127.0.0.1:9181
make start
```

2. In jailed MCP process/container, point to host discovery URL:

```bash
export JANUS_PUBLIC_BASE_URL=http://host.docker.internal:9181
# optional:
# export JANUS_PUBLIC_AUTH_BEARER=...
janus-mcp
```

In this mode, no host control socket mount is required in the jail.

### MCP host config example

```json
{
  "mcpServers": {
    "janus": {
      "command": "janus-mcp",
      "args": [],
      "env": {
        "JANUS_PUBLIC_BASE_URL": "http://host.docker.internal:9181"
      }
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

MCP behavior:
- public discovery only (`GET /health`, `GET /v1/config`),
- no session creation,
- no secret/token returns,
- deterministic non-LLM policy metadata from janusd.

MCP tools exposed:
- `janus.health`
- `janus.capabilities`
- `janus.discovery` (protocol/resource availability, unavailable gaps, deterministic model metadata)
- `janus.safety`

MCP resources exposed:
- `janus://discovery/protocols`
- `janus://discovery/resources`
- `janus://discovery/summary`

## Safety Model

- Upstream credentials stay on host and are never returned by API.
- MCP companion is read-only metadata only.
- Janus daemon policy evaluation is deterministic and non-LLM.
- LLMs discover capabilities/resources via MCP; users do not need to construct proxy calls manually.

Important deployment assumption:
- sandboxed agents must not have filesystem access to the host control socket path.
- if MCP runs inside a jail, use `JANUS_PUBLIC_BASE_URL` and network policy instead of socket mounts.

## Environment Variables

Example files:
- `.env.example` (local host run)
- `.env.docker.example` (docker deploy via `make deploy`)

Core:
- `JANUS_PROXY_BIND` (default `127.0.0.1:9080`)
- `JANUS_CONTROL_SOCKET` (default `/tmp/janusd-control.sock`)
- `JANUS_DISCOVERY_BIND` (optional read-only discovery listener; example `127.0.0.1:9181`)
- `JANUS_DEFAULT_TTL_SECONDS` (default `3600`)
- `JANUS_DEFAULT_CAPABILITIES` (default `http_proxy,git_http`)
- `JANUS_ALLOWED_HOSTS` (default `github.com,api.github.com,gitlab.com`)

MCP discovery transport:
- `JANUS_PUBLIC_BASE_URL` (optional; when set, `janus-mcp` uses network instead of unix socket)
- `JANUS_PUBLIC_AUTH_BEARER` (optional bearer token for discovery API)
- `JANUS_CONTROL_SOCKET` (used by `janus-mcp` only when `JANUS_PUBLIC_BASE_URL` is unset)

Git auth:
- `JANUS_GIT_HTTP_PASSWORD` or `JANUS_GIT_HTTP_TOKEN`
- `JANUS_GIT_HTTP_USERNAME` (default `x-access-token`)
- `JANUS_GIT_HTTP_HOSTS` (default `github.com`)
- `JANUS_GIT_SSH_AUTH_SOCK` (default `/var/run/janus/ssh-agent.sock`)
- `JANUS_GIT_SSH_PRIVATE_KEY_FILE` (optional readable private key file path)
- `JANUS_GIT_SSH_PRIVATE_KEY_B64` (optional base64-encoded private key)
- `JANUS_GIT_SSH_PRIVATE_KEY` (optional inline PEM text)

Kubernetes tooling (optional):
- `JANUS_KUBECONFIG`

UI:
- `JANUS_NO_BANNER=1` disables startup banner.

## License And Warranty

Licensed under MIT. See [LICENSE](./LICENSE).

This software is provided **"AS IS"**, without warranty of any kind, express or implied.
