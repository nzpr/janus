# Janus Go Reimplementation

This folder contains a Go reimplementation of Janus with the same strict model:

- `cmd/janusd`: host daemon (proxy + control API + typed adapters)
- `cmd/janus-mcp`: read-only MCP companion for capability discovery

## Build

```bash
cd go
go build ./...
```

## Run Daemon

```bash
cd go
go run ./cmd/janusd
```

Defaults:
- proxy bind: `127.0.0.1:9080`
- control socket: `/tmp/janusd-control.sock`

## Run MCP Companion

```bash
cd go
go run ./cmd/janus-mcp
```

MCP tools exposed:
- `janus.health`
- `janus.capabilities`
- `janus.safety`

No session creation, token issuance, or secret read APIs are exposed via MCP.

## Capabilities

Known capabilities:
- `http_proxy`
- `git_http`
- `git_ssh`
- `postgres_wire`
- `mysql_wire`
- `redis`
- `mongodb`
- `amqp`
- `kafka`
- `nats`
- `mqtt`
- `ldap`
- `sftp`
- `smb`
- `postgres_query`
- `deploy_kubectl`
- `deploy_helm`
- `deploy_terraform`

Default session capabilities:
- `http_proxy`
- `git_http`

`git_ssh` notes:
- Janus issues `GIT_SSH_COMMAND` in session env.
- Janus also issues `SSH_AUTH_SOCK` when `JANUS_GIT_SSH_AUTH_SOCK` (or `SSH_AUTH_SOCK`) is configured.
- SSH is tunneled through Janus CONNECT with session token auth.
- `git_ssh` only authorizes CONNECT on port `22` and still enforces `allowed_hosts`.
- Runtime must have `/bin/bash` (used by injected ProxyCommand).

Protocol CONNECT capability notes (first iteration):
- `postgres_wire` -> port `5432`
- `mysql_wire` -> port `3306`
- `redis` -> port `6379`
- `mongodb` -> port `27017`
- `amqp` -> port `5672`
- `kafka` -> port `9092`
- `nats` -> port `4222`
- `mqtt` -> ports `1883`, `8883`
- `ldap` -> ports `389`, `636`
- `sftp` -> port `22`
- `smb` -> port `445`

## Environment Variables

Daemon uses the same core env model as Rust implementation:
- `JANUS_PROXY_BIND`
- `JANUS_CONTROL_SOCKET`
- `JANUS_DEFAULT_TTL_SECONDS`
- `JANUS_DEFAULT_CAPABILITIES`
- `JANUS_ALLOWED_HOSTS`
- `JANUS_GIT_HTTP_PASSWORD` or `JANUS_GIT_HTTP_TOKEN`
- `JANUS_GIT_HTTP_USERNAME`
- `JANUS_GIT_HTTP_HOSTS`
- `JANUS_GIT_SSH_AUTH_SOCK` (optional agent socket path exposed to `git_ssh` sessions)
- `JANUS_POSTGRES_HOST`, `JANUS_POSTGRES_PORT`, `JANUS_POSTGRES_USER`, `JANUS_POSTGRES_DATABASE`, `JANUS_POSTGRES_PASSWORD`
- `JANUS_KUBECONFIG`
- `JANUS_NO_BANNER=1`
